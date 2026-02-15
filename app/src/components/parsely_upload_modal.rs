//! Parsely upload modal component for setting visibility and notes before upload.

use dioxus::prelude::*;

use super::toast::{use_toast, ToastSeverity};
use crate::api;

/// Type of upload being performed
#[derive(Clone, PartialEq)]
pub enum ParselyUploadType {
    /// Upload entire file
    File { path: String, filename: String },
    /// Upload specific encounter
    Encounter {
        path: String,
        encounter_name: String,
        enc_id: u64,
        start_line: u64,
        end_line: u64,
        area_entered_line: Option<u64>,
    },
}

/// Request to upload to Parsely
#[derive(Clone)]
struct ParselyUploadRequest {
    upload_type: ParselyUploadType,
    visibility: u8,
    notes: String,
}

/// Global manager for Parsely upload modal
#[derive(Clone, Copy)]
pub struct ParselyUploadManager {
    request: Signal<Option<ParselyUploadRequest>>,
}

impl ParselyUploadManager {
    /// Create a new upload manager
    pub fn new() -> Self {
        Self {
            request: Signal::new(None),
        }
    }

    /// Open modal for file upload
    pub fn open_file(&mut self, path: String, filename: String) {
        *self.request.write() = Some(ParselyUploadRequest {
            upload_type: ParselyUploadType::File { path, filename },
            visibility: 1, // Default to Public
            notes: String::new(),
        });
    }

    /// Open modal for encounter upload
    pub fn open_encounter(
        &mut self,
        path: String,
        encounter_name: String,
        enc_id: u64,
        start_line: u64,
        end_line: u64,
        area_entered_line: Option<u64>,
    ) {
        *self.request.write() = Some(ParselyUploadRequest {
            upload_type: ParselyUploadType::Encounter {
                path,
                encounter_name,
                enc_id,
                start_line,
                end_line,
                area_entered_line,
            },
            visibility: 1, // Default to Public
            notes: String::new(),
        });
    }

    /// Close modal without uploading
    pub fn close(&mut self) {
        *self.request.write() = None;
    }
}

impl Default for ParselyUploadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize Parsely upload provider at app root
pub fn use_parsely_upload_provider() -> ParselyUploadManager {
    use_context_provider(ParselyUploadManager::new)
}

/// Get the Parsely upload manager from context
pub fn use_parsely_upload() -> ParselyUploadManager {
    use_context::<ParselyUploadManager>()
}

/// Parsely upload modal component
#[component]
pub fn ParselyUploadModal() -> Element {
    let mut manager = use_parsely_upload();
    let mut toast = use_toast();

    let request = manager.request.read();
    let mut is_uploading = use_signal(|| false);

    let Some(req) = request.as_ref() else {
        return rsx! {};
    };

    // Get display name for the upload
    let display_name = match &req.upload_type {
        ParselyUploadType::File { filename, .. } => filename.clone(),
        ParselyUploadType::Encounter { encounter_name, .. } => encounter_name.clone(),
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| {
                if !is_uploading() {
                    manager.close();
                }
            },
            div {
                class: "parsely-upload-modal",
                onclick: move |e| e.stop_propagation(),

                // Header
                div { class: "modal-header",
                    h3 { "Parsely Upload" }
                    if !is_uploading() {
                        button {
                            class: "btn btn-close",
                            onclick: move |_| manager.close(),
                            "X"
                        }
                    }
                }

                // Content
                div { class: "modal-content",
                    p { class: "upload-message",
                        "Uploading "
                        strong { "{display_name}" }
                    }

                    // Notes field
                    label { r#for: "parsely-notes", class: "field-header", "Optional Note" }
                    textarea {
                        id: "parsely-notes",
                        class: "parsely-upload-notes",
                        placeholder: "Add a note about this upload...",
                        rows: 3,
                        disabled: is_uploading(),
                        value: "{req.notes}",
                        oninput: move |e| {
                            manager.request.write().as_mut().unwrap().notes = e.value();
                        }
                    }

                    // Visibility radio buttons
                    div { class: "parsely-upload-visibility",
                        label { class: "field-header", "Visibility" }
                        div { class: "radio-group",
                            label { class: "radio-option",
                                input {
                                    r#type: "radio",
                                    name: "visibility",
                                    value: "1",
                                    checked: req.visibility == 1,
                                    disabled: is_uploading(),
                                    onchange: move |_| {
                                        manager.request.write().as_mut().unwrap().visibility = 1;
                                    }
                                }
                                " Public"
                            }
                            label { class: "radio-option",
                                input {
                                    r#type: "radio",
                                    name: "visibility",
                                    value: "2",
                                    checked: req.visibility == 2,
                                    disabled: is_uploading(),
                                    onchange: move |_| {
                                        manager.request.write().as_mut().unwrap().visibility = 2;
                                    }
                                }
                                " Guild only"
                            }
                            label { class: "radio-option",
                                input {
                                    r#type: "radio",
                                    name: "visibility",
                                    value: "0",
                                    checked: req.visibility == 0,
                                    disabled: is_uploading(),
                                    onchange: move |_| {
                                        manager.request.write().as_mut().unwrap().visibility = 0;
                                    }
                                }
                                " Private"
                            }
                        }
                    }
                }

                // Footer with action buttons
                div { class: "modal-footer",
                    button {
                        class: "btn btn-secondary",
                        disabled: is_uploading(),
                        onclick: move |_| manager.close(),
                        "Cancel"
                    }
                    button {
                        class: "btn btn-primary",
                        disabled: is_uploading(),
                        onclick: move |_| {
                            let upload_req = manager.request.read().clone().unwrap();
                            let visibility = upload_req.visibility;
                            let notes = if upload_req.notes.is_empty() {
                                None
                            } else {
                                Some(upload_req.notes.clone())
                            };

                            is_uploading.set(true);

                            spawn(async move {
                                let is_file_upload = matches!(&upload_req.upload_type, ParselyUploadType::File { .. });
                                let upload_path = match &upload_req.upload_type {
                                    ParselyUploadType::File { path, .. } => path.clone(),
                                    ParselyUploadType::Encounter { path, .. } => path.clone(),
                                };
                                let display_name = match &upload_req.upload_type {
                                    ParselyUploadType::File { filename, .. } => filename.clone(),
                                    ParselyUploadType::Encounter { encounter_name, .. } => encounter_name.clone(),
                                };

                                let result = match upload_req.upload_type {
                                    ParselyUploadType::File { path, .. } => {
                                        api::upload_to_parsely(&path, visibility, notes).await
                                    }
                                    ParselyUploadType::Encounter {
                                        path,
                                        enc_id,
                                        start_line,
                                        end_line,
                                        area_entered_line,
                                        ..
                                    } => {
                                        let resp = api::upload_encounter_to_parsely(
                                            &path,
                                            start_line,
                                            end_line,
                                            area_entered_line,
                                            visibility,
                                            notes,
                                        ).await;

                                        // If successful, persist the link
                                        if let Ok(ref r) = resp {
                                            if r.success {
                                                if let Some(ref link) = r.link {
                                                    let _ = api::set_encounter_parsely_link(enc_id, link).await;

                                                    // Emit event to refresh encounter list
                                                    if let Some(window) = web_sys::window() {
                                                        if let Ok(event) = web_sys::CustomEvent::new("parsely-upload-success") {
                                                            let _ = window.dispatch_event(&event);
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        resp
                                    }
                                };

                                is_uploading.set(false);
                                manager.close();

                                // Emit event with upload result for UI updates (file uploads only)
                                // Individual encounter uploads have their own state management via
                                // parsely_link field and parsely-upload-success event
                                if is_file_upload {
                                    if let Some(window) = web_sys::window() {
                                        let global = js_sys::global();
                                        match &result {
                                            Ok(resp) if resp.success => {
                                                let link = resp.link.clone().unwrap_or_default();
                                                let data = format!("{}|true|{}", upload_path, link);
                                                js_sys::Reflect::set(&global, &"__parsely_upload_result".into(), &data.into()).ok();
                                            }
                                            Ok(resp) => {
                                                let err = resp.error.clone().unwrap_or_else(|| "Upload failed".to_string());
                                                let data = format!("{}|false|{}", upload_path, err);
                                                js_sys::Reflect::set(&global, &"__parsely_upload_result".into(), &data.into()).ok();
                                            }
                                            Err(e) => {
                                                let data = format!("{}|false|{}", upload_path, e);
                                                js_sys::Reflect::set(&global, &"__parsely_upload_result".into(), &data.into()).ok();
                                            }
                                        }

                                        if let Ok(event) = web_sys::Event::new("parsely-upload-complete") {
                                            let _ = window.dispatch_event(&event);
                                        }
                                    }
                                }

                                match result {
                                    Ok(resp) if resp.success => {
                                        if let Some(link) = resp.link {
                                            toast.show_with_link(
                                                format!("Uploaded {}: ", display_name),
                                                link,
                                                ToastSeverity::Success,
                                                15_000,
                                            );
                                        } else {
                                            toast.show(
                                                format!("Uploaded {}", display_name),
                                                ToastSeverity::Success,
                                            );
                                        }
                                    }
                                    Ok(resp) => {
                                        let err = resp.error.unwrap_or_else(|| "Upload failed".to_string());
                                        toast.show(format!("Upload failed: {}", err), ToastSeverity::Critical);
                                    }
                                    Err(e) => {
                                        toast.show(format!("Upload error: {}", e), ToastSeverity::Critical);
                                    }
                                }
                            });
                        },
                        if is_uploading() {
                            i { class: "fa-solid fa-spinner fa-spin" }
                            " Uploading..."
                        } else {
                            i { class: "fa-solid fa-cloud-arrow-up" }
                            " Upload"
                        }
                    }
                }
            }
        }
    }
}
