use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{
    AbsInfo, AbsoluteAxisType, AttributeSet, EventType, InputEvent, Key, Synchronization,
    UinputAbsSetup,
};
use std::error::Error;

pub struct MolexVirtualDevice {
    device: VirtualDevice,
}

impl MolexVirtualDevice {
    // Inicializa el dispositivo virtual en el sistema operativo
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut keys = AttributeSet::new();
        keys.insert(Key::BTN_LEFT);
        keys.insert(Key::BTN_RIGHT);

        // Ajustamos la pantalla base a 1920x1080 (Se puede hacer dinámico en el futuro)
        let x_axis = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_X,
            AbsInfo::new(0, 0, 1920, 0, 0, 0),
        );
        let y_axis = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_Y,
            AbsInfo::new(0, 0, 1080, 0, 0, 0),
        );

        let device = VirtualDeviceBuilder::new()?
            .name("Molex Virtual Mouse")
            .with_keys(&keys)?
            .with_absolute_axis(&x_axis)?
            .with_absolute_axis(&y_axis)?
            .build()?;

        Ok(Self { device })
    }

    // Inyecta las coordenadas absolutas al Kernel
    pub fn move_mouse(&mut self, x: i32, y: i32) -> Result<(), Box<dyn Error>> {
        let events = [
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, x),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, y),
            InputEvent::new(EventType::SYNCHRONIZATION, Synchronization::SYN_REPORT.0, 0),
        ];
        self.device.emit(&events)?;
        Ok(())
    }

    // Simula presionar o soltar un botón
    pub fn click_mouse(&mut self, button_key: Key, is_down: bool) -> Result<(), Box<dyn Error>> {
        let value = if is_down { 1 } else { 0 };
        let events = [
            InputEvent::new(EventType::KEY, button_key.code(), value),
            InputEvent::new(EventType::SYNCHRONIZATION, Synchronization::SYN_REPORT.0, 0),
        ];
        self.device.emit(&events)?;
        Ok(())
    }
}
