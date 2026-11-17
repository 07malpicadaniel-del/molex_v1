use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{
    AbsInfo, AbsoluteAxisType, AttributeSet, EventType, InputEvent, Key, RelativeAxisType,
    Synchronization, UinputAbsSetup,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn parse(token: &str) -> Self {
        let lower = token.trim().to_ascii_lowercase();
        match lower.as_str() {
            "right" | "2" | "true" => Self::Right,
            "middle" | "1" | "mid" => Self::Middle,
            _ => Self::Left,
        }
    }

    pub fn to_evdev_key(self) -> Key {
        match self {
            Self::Left => Key::BTN_LEFT,
            Self::Right => Key::BTN_RIGHT,
            Self::Middle => Key::BTN_MIDDLE,
        }
    }
}

pub struct MolexMouse {
    device: VirtualDevice,
    max_x: i32,
    max_y: i32,
}

impl MolexMouse {
    pub fn new(max_x: i32, max_y: i32) -> std::io::Result<Self> {
        let mut keys = AttributeSet::new();
        keys.insert(Key::BTN_LEFT);
        keys.insert(Key::BTN_RIGHT);
        keys.insert(Key::BTN_MIDDLE);

        // Teclado completo (Keycodes de Linux 1..=255, incluyendo Super/Meta, Ctrl, Alt)
        for code in 1..=255 {
            keys.insert(Key::new(code));
        }

        let abs_x = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_X,
            AbsInfo::new(0, 0, max_x, 0, 0, 1),
        );
        let abs_y = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_Y,
            AbsInfo::new(0, 0, max_y, 0, 0, 1),
        );

        let mut rel_axes = AttributeSet::new();
        rel_axes.insert(RelativeAxisType::REL_WHEEL);
        rel_axes.insert(RelativeAxisType::REL_HWHEEL);

        let device = VirtualDeviceBuilder::new()?
            .name("Molex Virtual Input Device")
            .with_keys(&keys)?
            .with_absolute_axis(&abs_x)?
            .with_absolute_axis(&abs_y)?
            .with_relative_axes(&rel_axes)?
            .build()?;

        Ok(Self {
            device,
            max_x,
            max_y,
        })
    }

    pub fn move_mouse(&mut self, x_percent: f32, y_percent: f32) -> std::io::Result<()> {
        let x = (x_percent.clamp(0.0, 1.0) * self.max_x as f32).round() as i32;
        let y = (y_percent.clamp(0.0, 1.0) * self.max_y as f32) as i32;

        self.device.emit(&[
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, x),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, y),
            InputEvent::new(
                EventType::SYNCHRONIZATION,
                Synchronization::SYN_REPORT.0,
                0,
            ),
        ])
    }

    pub fn mouse_button(&mut self, button: MouseButton, pressed: bool) -> std::io::Result<()> {
        let key = button.to_evdev_key();
        let state = if pressed { 1 } else { 0 };
        self.device.emit(&[
            InputEvent::new(EventType::KEY, key.code(), state),
            InputEvent::new(
                EventType::SYNCHRONIZATION,
                Synchronization::SYN_REPORT.0,
                0,
            ),
        ])
    }

    pub fn click(&mut self, button: MouseButton) -> std::io::Result<()> {
        self.mouse_button(button, true)?;
        self.mouse_button(button, false)
    }

    pub fn scroll(&mut self, dx: i32, dy: i32) -> std::io::Result<()> {
        let mut events = Vec::with_capacity(3);
        if dy != 0 {
            events.push(InputEvent::new(
                EventType::RELATIVE,
                RelativeAxisType::REL_WHEEL.0,
                dy,
            ));
        }
        if dx != 0 {
            events.push(InputEvent::new(
                EventType::RELATIVE,
                RelativeAxisType::REL_HWHEEL.0,
                dx,
            ));
        }
        if events.is_empty() {
            return Ok(());
        }
        events.push(InputEvent::new(
            EventType::SYNCHRONIZATION,
            Synchronization::SYN_REPORT.0,
            0,
        ));
        self.device.emit(&events)
    }

    pub fn send_key(&mut self, key_code: u16, state: i32) -> std::io::Result<()> {
        if key_code == 0 || key_code > 255 {
            return Ok(());
        }
        let clamped_state = state.clamp(0, 2);
        self.device.emit(&[
            InputEvent::new(EventType::KEY, key_code, clamped_state),
            InputEvent::new(
                EventType::SYNCHRONIZATION,
                Synchronization::SYN_REPORT.0,
                0,
            ),
        ])
    }
}

pub fn create_input_device() -> std::io::Result<MolexMouse> {
    MolexMouse::new(65535, 65535)
}
