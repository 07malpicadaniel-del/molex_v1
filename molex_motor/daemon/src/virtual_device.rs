use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AbsoluteAxisType, AbsInfo, AttributeSet, EventType, InputEvent, Key, UinputAbsSetup};

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

        // Configuramos un área táctil absoluta de altísima resolución
        let abs_x = UinputAbsSetup::new(AbsoluteAxisType::ABS_X, AbsInfo::new(0, 0, max_x, 0, 0, 1));
        let abs_y = UinputAbsSetup::new(AbsoluteAxisType::ABS_Y, AbsInfo::new(0, 0, max_y, 0, 0, 1));

        let device = VirtualDeviceBuilder::new()?
            .name("Molex Virtual Touchpad")
            .with_keys(&keys)?
            .with_absolute_axis(&abs_x)?
            .with_absolute_axis(&abs_y)?
            .build()?;

        Ok(Self { device, max_x, max_y })
    }

    pub fn move_mouse(&mut self, x_percent: f32, y_percent: f32) -> std::io::Result<()> {
        let x = (x_percent * self.max_x as f32) as i32;
        let y = (y_percent * self.max_y as f32) as i32;
        
        // Inyectamos coordenadas y forzamos la sincronización inmediata del kernel
        self.device.emit(&[
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, x),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, y),
            InputEvent::new(EventType::SYNCHRONIZATION, evdev::Synchronization::SYN_REPORT.0, 0),
        ])
    }

    pub fn click(&mut self, is_right: bool) -> std::io::Result<()> {
        let btn = if is_right { Key::BTN_RIGHT } else { Key::BTN_LEFT };
        
        // Clic abajo
        self.device.emit(&[
            InputEvent::new(EventType::KEY, btn.code(), 1),
            InputEvent::new(EventType::SYNCHRONIZATION, evdev::Synchronization::SYN_REPORT.0, 0),
        ])?;
        // Clic arriba
        self.device.emit(&[
            InputEvent::new(EventType::KEY, btn.code(), 0),
            InputEvent::new(EventType::SYNCHRONIZATION, evdev::Synchronization::SYN_REPORT.0, 0),
        ])
    }
}
