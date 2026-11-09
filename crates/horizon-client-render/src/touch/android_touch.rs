//! Controles de toque do Android no estilo Bedrock.
//!
//! - D-pad e botões VISÍVEIS (a lista de formas vem de [`TouchControls::shapes`]).
//! - Multitoque: cada dedo mantém sua função até soltar.
//! - Opcional: botão "AUTO / ON / OFF" no topo da tela. Em AUTO os controles somem
//!   quando você usa teclado ou mouse físico e voltam ao tocar na tela.
//! - Barra rápida: tocar num slot seleciona (teclas 1..9).
//! Os toques continuam virando teclas/cliques sintéticos, como antes.
use std::collections::BTreeMap;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, Touch, TouchPhase},
    keyboard::KeyCode,
};

const DIGITS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];
/// Frente, trás, esquerda, direita, correr.
const MOVE_KEYS: [KeyCode; 5] = [
    KeyCode::KeyW,
    KeyCode::KeyS,
    KeyCode::KeyA,
    KeyCode::KeyD,
    KeyCode::ControlLeft,
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// Visível; some com teclado/mouse físico e volta ao tocar na tela.
    Auto,
    /// Sempre visível.
    On,
    /// Desligado (só teclado/mouse no jogo; menus ainda aceitam toque).
    Off,
}
impl Mode {
    fn next(self) -> Self {
        match self {
            Mode::Auto => Mode::On,
            Mode::On => Mode::Off,
            Mode::Off => Mode::Auto,
        }
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Mode::Auto => "TOQUE: AUTO",
            Mode::On => "TOQUE: LIGADO",
            Mode::Off => "TOQUE: OFF",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Move,
    Look,
    Key(KeyCode),
    Button(MouseButton),
    Pointer,
    Toggle,
}
struct Finger {
    role: Role,
    position: PhysicalPosition<f64>,
}

pub(super) enum Action {
    Key(KeyCode, bool),
    Button(MouseButton, bool),
    Look(f64, f64),
    Pointer(PhysicalPosition<f64>, Option<ElementState>),
}

/// Retângulo em pixels da janela.
#[derive(Clone, Copy)]
pub(super) struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
impl Rect {
    fn around(cx: f64, cy: f64, half: f64) -> Self {
        Self {
            x: cx - half,
            y: cy - half,
            w: 2.0 * half,
            h: 2.0 * half,
        }
    }
    fn grown(self, by: f64) -> Self {
        Self {
            x: self.x - by,
            y: self.y - by,
            w: self.w + 2.0 * by,
            h: self.h + 2.0 * by,
        }
    }
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// O que o renderizador da HUD precisa desenhar.
pub(super) enum Shape {
    /// Retângulo preenchido, cor RGBA 0..1.
    Fill(Rect, [f32; 4]),
    /// Texto centralizado no retângulo.
    Label(Rect, &'static str, [f32; 4]),
}

#[derive(Clone, Copy)]
enum Target {
    Key(KeyCode),
    Button(MouseButton),
}
impl Target {
    fn role(self) -> Role {
        match self {
            Target::Key(k) => Role::Key(k),
            Target::Button(b) => Role::Button(b),
        }
    }
}
struct Slot {
    target: Target,
    rect: Rect,
    label: &'static str,
}
/// Posições dos controles; usadas tanto para desenhar quanto para detectar toques.
struct Layout {
    u: f64,
    pad_center: (f64, f64),
    pad: Rect,
    slots: Vec<Slot>,
    toggle: Rect,
    hotbar: Rect,
    gui: f64,
}
fn layout(size: [u32; 2], gui: Option<f64>) -> Layout {
    let w = f64::from(size[0].max(1));
    let h = f64::from(size[1].max(1));
    let u = (h * 0.085).max(36.0);
    // Escala automática da GUI do Minecraft (maior inteiro que cabe em 320x240).
    let gui = gui.unwrap_or_else(|| (w / 320.0).min(h / 240.0).floor().max(1.0));
    let pad_center = (2.4 * u, h - 2.4 * u);
    let slot = |target: Target, cx: f64, cy: f64, half: f64, label: &'static str| Slot {
        target,
        rect: Rect::around(cx, cy, half),
        label,
    };
    Layout {
        u,
        pad_center,
        pad: Rect::around(pad_center.0, pad_center.1, 2.0 * u),
        slots: vec![
            slot(Target::Key(KeyCode::Space), w - 1.7 * u, h - 1.7 * u, 0.95 * u, "PULAR"),
            slot(Target::Key(KeyCode::ShiftLeft), w - 3.8 * u, h - 1.3 * u, 0.75 * u, "AGACH"),
            slot(Target::Button(MouseButton::Left), w - 1.7 * u, h - 3.9 * u, 0.8 * u, "BATER"),
            slot(Target::Button(MouseButton::Right), w - 3.8 * u, h - 3.2 * u, 0.8 * u, "USAR"),
            slot(Target::Key(KeyCode::KeyE), w - 1.0 * u, 0.9 * u, 0.6 * u, "INV"),
            slot(Target::Key(KeyCode::Escape), 1.0 * u, 0.9 * u, 0.6 * u, "II"),
        ],
        toggle: Rect {
            x: w / 2.0 - 1.6 * u,
            y: 0.0,
            w: 3.2 * u,
            h: 0.7 * u,
        },
        hotbar: Rect {
            x: (w - 182.0 * gui) / 2.0,
            y: h - 22.0 * gui,
            w: 182.0 * gui,
            h: 22.0 * gui,
        },
        gui,
    }
}

pub(super) struct TouchControls {
    fingers: BTreeMap<u64, Finger>,
    /// W, S, A, D, Ctrl atualmente pressionados pelo D-pad.
    held: [bool; 5],
    mode: Mode,
    /// Só importa em `Mode::Auto`.
    visible: bool,
    /// 0.1..1.0, multiplica a transparência de tudo.
    pub(super) opacity: f32,
    /// Escala da GUI, se quiser forçar a mesma da HUD (`HudBatch::new(size).scale`).
    pub(super) gui_scale: Option<f64>,
    /// Ligue enquanto o jogo reenvia eventos sintéticos de mouse gerados por toque.
    pub(super) synthetic: bool,
}
impl Default for TouchControls {
    fn default() -> Self {
        Self {
            fingers: BTreeMap::new(),
            held: [false; 5],
            mode: Mode::Auto,
            visible: true,
            opacity: 1.0,
            gui_scale: None,
            synthetic: false,
        }
    }
}

impl TouchControls {
    pub(super) fn mode(&self) -> Mode {
        self.mode
    }
    /// Os controles de jogo estão ligados e visíveis agora?
    pub(super) fn active(&self) -> bool {
        self.mode != Mode::Off && (self.mode == Mode::On || self.visible)
    }
    /// Chame quando chegar teclado/mouse REAL (não os sintéticos do próprio toque).
    pub(super) fn note_hardware(&mut self) {
        if !self.synthetic && self.mode == Mode::Auto {
            self.visible = false;
        }
    }

    pub(super) fn clear(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        for finger in self.fingers.values() {
            match finger.role {
                Role::Key(key) => actions.push(Action::Key(key, false)),
                Role::Button(button) => actions.push(Action::Button(button, false)),
                Role::Pointer => actions.push(Action::Pointer(
                    finger.position,
                    Some(ElementState::Released),
                )),
                _ => {}
            }
        }
        for (key, held) in MOVE_KEYS.into_iter().zip(self.held) {
            if held {
                actions.push(Action::Key(key, false));
            }
        }
        self.fingers.clear();
        self.held = [false; 5];
        actions
    }

    pub(super) fn update(
        &mut self,
        touch: Touch,
        size: [u32; 2],
        playing: bool,
        closable: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let lay = layout(size, self.gui_scale);
        let w = f64::from(size[0].max(1));
        let h = f64::from(size[1].max(1));
        let (x, y) = (touch.location.x, touch.location.y);
        if touch.phase == TouchPhase::Started {
            // Tocar na tela traz os controles de volta depois de usar teclado/mouse.
            if playing && !self.visible && self.mode == Mode::Auto {
                self.visible = true;
            }
            let role = if closable && x / w > 0.92 && y / h < 0.1 {
                Some(Role::Key(KeyCode::Escape))
            } else if !playing {
                Some(Role::Pointer)
            } else if lay.toggle.contains(x, y) {
                Some(Role::Toggle)
            } else if !self.active() {
                None
            } else if lay.hotbar.contains(x, y) {
                let i = ((x - lay.hotbar.x - lay.gui) / (20.0 * lay.gui))
                    .floor()
                    .clamp(0.0, 8.0) as usize;
                Some(Role::Key(DIGITS[i]))
            } else if lay.pad.grown(0.3 * lay.u).contains(x, y) {
                Some(Role::Move)
            } else if let Some(s) = lay
                .slots
                .iter()
                .find(|s| s.rect.grown(0.12 * lay.u).contains(x, y))
            {
                Some(s.target.role())
            } else {
                Some(Role::Look)
            };
            let Some(role) = role else {
                return actions;
            };
            if role == Role::Toggle {
                // Solta tudo antes de trocar de modo, para nada ficar preso.
                actions.extend(self.clear());
                self.mode = self.mode.next();
                self.visible = true;
                self.fingers.insert(
                    touch.id,
                    Finger {
                        role,
                        position: touch.location,
                    },
                );
                return actions;
            }
            // Uma zona tem um dono só: soltar outro dedo não a libera.
            let occupied = self.fingers.values().any(|f| match (f.role, role) {
                (Role::Move, Role::Move)
                | (Role::Look, Role::Look)
                | (Role::Pointer, Role::Pointer) => true,
                (Role::Key(a), Role::Key(b)) => a == b,
                (Role::Button(a), Role::Button(b)) => a == b,
                _ => false,
            });
            if occupied {
                return actions;
            }
            match role {
                Role::Key(key) => actions.push(Action::Key(key, true)),
                Role::Button(button) => actions.push(Action::Button(button, true)),
                Role::Pointer => {
                    actions.push(Action::Pointer(touch.location, Some(ElementState::Pressed)))
                }
                _ => {}
            }
            self.fingers.insert(
                touch.id,
                Finger {
                    role,
                    position: touch.location,
                },
            );
        } else if let Some(finger) = self.fingers.get_mut(&touch.id) {
            let ended = matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled);
            match finger.role {
                Role::Look if !ended => actions.push(Action::Look(
                    touch.location.x - finger.position.x,
                    touch.location.y - finger.position.y,
                )),
                Role::Key(key) if ended => actions.push(Action::Key(key, false)),
                Role::Button(button) if ended => actions.push(Action::Button(button, false)),
                Role::Pointer => actions.push(Action::Pointer(
                    touch.location,
                    ended.then_some(ElementState::Released),
                )),
                _ => {}
            }
            finger.position = touch.location;
            if ended {
                self.fingers.remove(&touch.id);
            }
        }
        // D-pad: o dedo em relação ao centro; diagonais valem.
        // Arrastar além da borda de cima do D-pad = correr (Ctrl).
        let mut held = [false; 5];
        if let Some(finger) = self.fingers.values().find(|f| f.role == Role::Move) {
            let dx = finger.position.x - lay.pad_center.0;
            let dy = finger.position.y - lay.pad_center.1;
            let neutral = 0.45 * lay.u;
            held = [
                dy < -neutral,
                dy > neutral,
                dx < -neutral,
                dx > neutral,
                dy < -2.4 * lay.u,
            ];
        }
        for (i, key) in MOVE_KEYS.into_iter().enumerate() {
            if held[i] != self.held[i] {
                actions.push(Action::Key(key, held[i]));
            }
        }
        self.held = held;
        actions
    }

    /// Formas a desenhar por cima da HUD durante o jogo.
    /// O botão de modo aparece sempre (mais apagado quando os controles estão ocultos).
    pub(super) fn shapes(&self, size: [u32; 2]) -> Vec<Shape> {
        let lay = layout(size, self.gui_scale);
        let a = self.opacity.clamp(0.1, 1.0);
        let idle = [1.0, 1.0, 1.0, 0.22 * a];
        let lit = [1.0, 1.0, 1.0, 0.55 * a];
        let text = [1.0, 1.0, 1.0, 0.9 * a];
        let mut out = Vec::new();

        let dim = if self.active() { 1.0 } else { 0.5 };
        out.push(Shape::Fill(lay.toggle, [0.0, 0.0, 0.0, 0.35 * a * dim]));
        out.push(Shape::Label(
            lay.toggle,
            self.mode.label(),
            [1.0, 1.0, 1.0, 0.8 * a * dim],
        ));
        if !self.active() {
            return out;
        }

        // D-pad em cruz.
        let (cx, cy) = lay.pad_center;
        let arm = 1.35 * lay.u;
        let half = 0.62 * lay.u;
        out.push(Shape::Fill(Rect::around(cx, cy, 0.45 * lay.u), idle));
        for (dx, dy, label, i) in [
            (0.0, -arm, "^", 0),
            (0.0, arm, "v", 1),
            (-arm, 0.0, "<", 2),
            (arm, 0.0, ">", 3),
        ] {
            let r = Rect::around(cx + dx, cy + dy, half);
            out.push(Shape::Fill(r, if self.held[i] { lit } else { idle }));
            out.push(Shape::Label(r, label, text));
        }
        // Dica de correr: arraste para cima, além do D-pad.
        let sprint = Rect {
            x: cx - 1.2 * lay.u,
            y: cy - arm - half - 0.7 * lay.u,
            w: 2.4 * lay.u,
            h: 0.5 * lay.u,
        };
        out.push(Shape::Fill(sprint, if self.held[4] { lit } else { idle }));
        out.push(Shape::Label(sprint, "CORRER", text));

        for s in &lay.slots {
            let pressed = self.fingers.values().any(|f| f.role == s.target.role());
            out.push(Shape::Fill(s.rect, if pressed { lit } else { idle }));
            out.push(Shape::Label(s.rect, s.label, text));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SIZE: [u32; 2] = [1000, 1000];
    fn touch(id: u64, phase: TouchPhase, x: f64, y: f64) -> Touch {
        Touch {
            device_id: winit::event::DeviceId::dummy(),
            phase,
            location: PhysicalPosition::new(x, y),
            force: None,
            id,
        }
    }
    fn send(c: &mut TouchControls, id: u64, phase: TouchPhase, x: f64, y: f64) -> Vec<Action> {
        c.update(touch(id, phase, x, y), SIZE, true, false)
    }

    #[test]
    fn cancelling_movement_releases_each_held_direction() {
        let mut controls = TouchControls::default();
        let press = send(&mut controls, 1, TouchPhase::Started, 120.0, 700.0);
        assert!(press.iter().any(|a| matches!(a, Action::Key(KeyCode::KeyW, true))));
        assert!(press.iter().any(|a| matches!(a, Action::Key(KeyCode::KeyA, true))));
        let release = send(&mut controls, 1, TouchPhase::Cancelled, 120.0, 700.0);
        assert_eq!(release.len(), 2);
        assert!(release.iter().all(|a| matches!(a, Action::Key(_, false))));
    }

    #[test]
    fn dragging_past_the_pad_sprints() {
        let mut controls = TouchControls::default();
        // Centro do D-pad (204, 796); 2.4u = 204 px acima dele.
        let press = send(&mut controls, 1, TouchPhase::Started, 204.0, 796.0);
        assert!(press.is_empty());
        let drag = send(&mut controls, 1, TouchPhase::Moved, 204.0, 560.0);
        assert!(drag.iter().any(|a| matches!(a, Action::Key(KeyCode::ControlLeft, true))));
    }

    #[test]
    fn lifting_look_finger_does_not_release_jump() {
        let mut controls = TouchControls::default();
        send(&mut controls, 1, TouchPhase::Started, 860.0, 860.0);
        send(&mut controls, 2, TouchPhase::Started, 600.0, 300.0);
        assert!(send(&mut controls, 2, TouchPhase::Ended, 650.0, 300.0).is_empty());
        assert!(
            controls
                .clear()
                .iter()
                .any(|a| matches!(a, Action::Key(KeyCode::Space, false)))
        );
    }

    #[test]
    fn hotbar_tap_selects_slot() {
        let mut controls = TouchControls::default();
        // gui = 3 em 1000x1000: barra em x 227..773, y 934..1000; slot 0 perto de x=260.
        let press = send(&mut controls, 1, TouchPhase::Started, 260.0, 960.0);
        assert!(press.iter().any(|a| matches!(a, Action::Key(KeyCode::Digit1, true))));
    }

    #[test]
    fn toggle_cycles_modes_and_off_ignores_gameplay_touches() {
        let mut controls = TouchControls::default();
        for expected in [Mode::On, Mode::Off] {
            send(&mut controls, 9, TouchPhase::Started, 500.0, 20.0);
            send(&mut controls, 9, TouchPhase::Ended, 500.0, 20.0);
            assert!(controls.mode() == expected);
        }
        assert!(send(&mut controls, 1, TouchPhase::Started, 860.0, 860.0).is_empty());
        send(&mut controls, 9, TouchPhase::Started, 500.0, 20.0);
        assert!(controls.mode() == Mode::Auto);
    }

    #[test]
    fn hardware_input_hides_in_auto_and_a_touch_brings_it_back() {
        let mut controls = TouchControls::default();
        controls.note_hardware();
        assert!(!controls.active());
        send(&mut controls, 1, TouchPhase::Started, 600.0, 300.0);
        assert!(controls.active());
        // Eventos sintéticos do próprio toque não contam como teclado/mouse.
        controls.synthetic = true;
        controls.note_hardware();
        assert!(controls.active());
    }
}