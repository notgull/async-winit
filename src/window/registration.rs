/*

`async-winit` is free software: you can redistribute it and/or modify it under the terms of one of
the following licenses:

- The GNU Affero General Public License as published by the Free Software Foundation, either version
  3 of the License, or (at your option) any later version.
- The Patron License at https://github.com/notgull/async-winit/blob/main/LICENSE-PATRON.md, for
  sponsors and contributors, who can ignore the copyleft provisions of the GNU AGPL for this project.

`async-winit` is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even
the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General
Public License and the Patron License for more details.

You should have received a copy of the GNU Affero General Public License and the corresponding Patron
License along with `async-winit`. If not, see <https://www.gnu.org/licenses/>.

*/

//! Registration of the window into the reactor.

use crate::dpi::PhysicalSize;
use crate::handler::Handler;
use crate::Event;

use winit::dpi::PhysicalPosition;
use winit::event::{
    AxisId, DeviceId, ElementState, Ime, ModifiersState, MouseButton, MouseScrollDelta, Touch,
    TouchPhase, WindowEvent,
};
use winit::window::Theme;

#[derive(Clone)]
pub struct KeyboardInput {
    pub device_id: DeviceId,
    pub input: winit::event::KeyboardInput,
    pub is_synthetic: bool,
}

#[derive(Clone)]
pub struct CursorMoved {
    pub device_id: DeviceId,
    pub position: PhysicalPosition<f64>,
}

#[derive(Clone)]
pub struct MouseWheel {
    pub device_id: DeviceId,
    pub delta: MouseScrollDelta,
    pub phase: TouchPhase,
}

#[derive(Clone)]
pub struct MouseInput {
    pub device_id: DeviceId,
    pub state: ElementState,
    pub button: MouseButton,
}

#[derive(Clone)]
pub struct TouchpadMagnify {
    pub device_id: DeviceId,
    pub delta: f64,
    pub phase: TouchPhase,
}

#[derive(Clone)]
pub struct TouchpadRotate {
    pub device_id: DeviceId,
    pub delta: f32,
    pub phase: TouchPhase,
}

#[derive(Clone)]
pub struct TouchpadPressure {
    pub device_id: DeviceId,
    pub pressure: f32,
    pub stage: i64,
}

#[derive(Clone)]
pub struct AxisMotion {
    pub device_id: DeviceId,
    pub axis: AxisId,
    pub value: f64,
}

pub struct ScaleFactor;

pub struct ScaleFactorChanging<'a> {
    pub scale_factor: f64,
    pub new_inner_size: &'a mut PhysicalSize<u32>,
}

#[derive(Clone)]
pub struct ScaleFactorChanged {
    pub scale_factor: f64,
    pub new_inner_size: PhysicalSize<u32>,
}

impl Event for ScaleFactor {
    type Clonable = ScaleFactorChanged;
    type Unique<'a> = ScaleFactorChanging<'a>;

    fn downgrade(unique: &mut Self::Unique<'_>) -> Self::Clonable {
        ScaleFactorChanged {
            scale_factor: unique.scale_factor,
            new_inner_size: *unique.new_inner_size,
        }
    }
}

pub(crate) struct Registration {
    /// `RedrawRequested`
    pub(crate) redraw_requested: Handler<()>,

    /// `Event::CloseRequested`.
    pub(crate) close_requested: Handler<()>,

    /// `Event::Resized`.
    pub(crate) resized: Handler<PhysicalSize<u32>>,

    /// `Event::Moved`.
    pub(crate) moved: Handler<PhysicalPosition<i32>>,

    /// `Event::Destroyed`.
    pub(crate) destroyed: Handler<()>,

    /// `Event::Focused`.
    pub(crate) focused: Handler<bool>,

    /// `Event::ReceivedCharacter`.
    pub(crate) received_character: Handler<char>,

    /// `Event::KeyboardInput`.
    pub(crate) keyboard_input: Handler<KeyboardInput>,

    /// `Event::ModifiersState`
    pub(crate) modifiers_changed: Handler<ModifiersState>,

    /// `Event::Ime`
    pub(crate) ime: Handler<Ime>,

    /// `Event::CursorMoved`
    pub(crate) cursor_moved: Handler<CursorMoved>,

    /// `Event::CursorEntered`
    pub(crate) cursor_entered: Handler<DeviceId>,

    /// `Event::CursorLeft`
    pub(crate) cursor_left: Handler<DeviceId>,

    /// `Event::MouseWheel`
    pub(crate) mouse_wheel: Handler<MouseWheel>,

    /// `Event::MouseInput`
    pub(crate) mouse_input: Handler<MouseInput>,

    /// `Event::TouchpadMagnify`
    pub(crate) touchpad_magnify: Handler<TouchpadMagnify>,

    /// `Event::SmartMagnify`.
    pub(crate) smart_magnify: Handler<DeviceId>,

    /// `Event::TouchpadRotate`
    pub(crate) touchpad_rotate: Handler<TouchpadRotate>,

    /// `Event::TouchpadPressure`
    pub(crate) touchpad_pressure: Handler<TouchpadPressure>,

    /// `Event::AxisMotion`
    pub(crate) axis_motion: Handler<AxisMotion>,

    /// `Event::Touch`
    pub(crate) touch: Handler<Touch>,

    /// `Event::ScaleFactorChanged`
    pub(crate) scale_factor_changed: Handler<ScaleFactor>,

    /// `Event::ThemeChanged`
    pub(crate) theme_changed: Handler<Theme>,

    /// `Event::Occluded`
    pub(crate) occluded: Handler<bool>,
}

impl Registration {
    pub(crate) fn new() -> Self {
        Self {
            close_requested: Handler::new(),
            resized: Handler::new(),
            redraw_requested: Handler::new(),
            moved: Handler::new(),
            destroyed: Handler::new(),
            focused: Handler::new(),
            keyboard_input: Handler::new(),
            received_character: Handler::new(),
            modifiers_changed: Handler::new(),
            ime: Handler::new(),
            cursor_entered: Handler::new(),
            cursor_left: Handler::new(),
            cursor_moved: Handler::new(),
            axis_motion: Handler::new(),
            scale_factor_changed: Handler::new(),
            smart_magnify: Handler::new(),
            theme_changed: Handler::new(),
            touch: Handler::new(),
            touchpad_magnify: Handler::new(),
            touchpad_pressure: Handler::new(),
            touchpad_rotate: Handler::new(),
            mouse_input: Handler::new(),
            mouse_wheel: Handler::new(),
            occluded: Handler::new(),
        }
    }

    pub(crate) async fn signal(&self, event: WindowEvent<'_>) {
        match event {
            WindowEvent::CloseRequested => self.close_requested.run_with(&mut ()).await,
            WindowEvent::Resized(mut size) => self.resized.run_with(&mut size).await,
            WindowEvent::Moved(mut posn) => self.moved.run_with(&mut posn).await,
            WindowEvent::AxisMotion {
                device_id,
                axis,
                value,
            } => {
                self.axis_motion
                    .run_with(&mut AxisMotion {
                        device_id,
                        axis,
                        value,
                    })
                    .await
            }
            WindowEvent::CursorEntered { mut device_id } => {
                self.cursor_entered.run_with(&mut device_id).await
            }
            WindowEvent::CursorLeft { mut device_id } => {
                self.cursor_left.run_with(&mut device_id).await
            }
            WindowEvent::CursorMoved {
                device_id,
                position,
                ..
            } => {
                self.cursor_moved
                    .run_with(&mut CursorMoved {
                        device_id,
                        position,
                    })
                    .await
            }
            WindowEvent::Destroyed => self.destroyed.run_with(&mut ()).await,
            WindowEvent::Focused(mut foc) => self.focused.run_with(&mut foc).await,
            WindowEvent::Ime(mut ime) => self.ime.run_with(&mut ime).await,
            WindowEvent::KeyboardInput {
                device_id,
                input,
                is_synthetic,
            } => {
                self.keyboard_input
                    .run_with(&mut KeyboardInput {
                        device_id,
                        input,
                        is_synthetic,
                    })
                    .await
            }
            WindowEvent::ModifiersChanged(mut mods) => {
                self.modifiers_changed.run_with(&mut mods).await
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
                ..
            } => {
                self.mouse_input
                    .run_with(&mut MouseInput {
                        device_id,
                        state,
                        button,
                    })
                    .await
            }
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
                ..
            } => {
                self.mouse_wheel
                    .run_with(&mut MouseWheel {
                        device_id,
                        delta,
                        phase,
                    })
                    .await
            }
            WindowEvent::Occluded(mut occ) => self.occluded.run_with(&mut occ).await,
            WindowEvent::ReceivedCharacter(mut ch) => {
                self.received_character.run_with(&mut ch).await
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                new_inner_size,
            } => {
                self.scale_factor_changed
                    .run_with(&mut ScaleFactorChanging {
                        scale_factor,
                        new_inner_size,
                    })
                    .await
            }
            WindowEvent::SmartMagnify { mut device_id } => {
                self.smart_magnify.run_with(&mut device_id).await
            }
            WindowEvent::ThemeChanged(mut theme) => self.theme_changed.run_with(&mut theme).await,
            WindowEvent::Touch(mut touch) => self.touch.run_with(&mut touch).await,
            WindowEvent::TouchpadMagnify {
                device_id,
                delta,
                phase,
            } => {
                self.touchpad_magnify
                    .run_with(&mut TouchpadMagnify {
                        device_id,
                        delta,
                        phase,
                    })
                    .await
            }
            WindowEvent::TouchpadPressure {
                device_id,
                pressure,
                stage,
            } => {
                self.touchpad_pressure
                    .run_with(&mut TouchpadPressure {
                        device_id,
                        pressure,
                        stage,
                    })
                    .await
            }
            WindowEvent::TouchpadRotate {
                device_id,
                delta,
                phase,
            } => {
                self.touchpad_rotate
                    .run_with(&mut TouchpadRotate {
                        device_id,
                        delta,
                        phase,
                    })
                    .await
            }
            _ => {}
        }
    }
}
