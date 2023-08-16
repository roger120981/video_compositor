use crate::{
    cef_ref::{CefRefData, CefStruct},
    render_handler::{RenderHandler, RenderHandlerWrapper},
};

/// [`Client`] is used for setting browser-specific handlers.
/// It should be defined for each browser session.
pub trait Client {
    type RenderHandlerType: RenderHandler;

    /// Returns browser's render handler.
    /// **Important:** it's called every time a frame is rendered
    fn render_handler(&self) -> Option<Self::RenderHandlerType> {
        None
    }
}

pub(crate) struct ClientWrapper<C: Client>(pub C);

impl<C: Client> CefStruct for ClientWrapper<C> {
    type CefType = chromium_sys::cef_client_t;

    fn cef_data(&self) -> Self::CefType {
        chromium_sys::cef_client_t {
            base: unsafe { std::mem::zeroed() },
            get_audio_handler: None,
            get_command_handler: None,
            get_context_menu_handler: None,
            get_dialog_handler: None,
            get_display_handler: None,
            get_download_handler: None,
            get_drag_handler: None,
            get_find_handler: None,
            get_focus_handler: None,
            get_frame_handler: None,
            get_permission_handler: None,
            get_jsdialog_handler: None,
            get_keyboard_handler: None,
            get_life_span_handler: None,
            get_load_handler: None,
            get_print_handler: None,
            get_render_handler: Some(Self::render_handler),
            get_request_handler: None,
            on_process_message_received: None,
        }
    }

    fn base_mut(cef_data: &mut Self::CefType) -> &mut chromium_sys::cef_base_ref_counted_t {
        &mut cef_data.base
    }
}

impl<C: Client> ClientWrapper<C> {
    extern "C" fn render_handler(
        self_: *mut chromium_sys::cef_client_t,
    ) -> *mut chromium_sys::cef_render_handler_t {
        // TODO: Make sure reference is created once
        unsafe {
            let self_ref = CefRefData::<Self>::from_cef(self_);
            match self_ref.0.render_handler() {
                Some(handler) => CefRefData::new_ptr(RenderHandlerWrapper(handler)),
                None => std::ptr::null_mut(),
            }
        }
    }
}