use std::sync::Arc;

use winit::{event_loop::EventLoop, window::Window};

#[allow(deprecated)]
fn main() {
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        event_loop
            .create_window(Window::default_attributes())
            .unwrap(),
    );
    let instance = vulkan_render::instance::InstanceForWindow::with_window(window);
    println!("successfully create instance");
    drop(instance);
}
