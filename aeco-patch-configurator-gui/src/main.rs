use aeco_patch_config::{error::PatchConfigError, generate_config};
use eframe::egui;
use eframe::epaint::Vec2;
use rfd::FileDialog;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::{sync::mpsc, thread};

/// Messages which the worker thread (for generating configs) can send back to
/// the GUI about the result of the operation.
enum MessageToGUI {
    Complete,
    Error(PatchConfigError),
}

struct PatchConfigApp {
    patch_folder: String,
    patch_output_folder: String,
    state_message: String,
    worker_rx: Option<Receiver<MessageToGUI>>,
    needs_repaint: bool,
}

impl PatchConfigApp {
    pub fn new() -> Self {
        Self {
            patch_folder: String::default(),
            patch_output_folder: String::default(),
            state_message: String::default(),
            worker_rx: None,
            needs_repaint: false,
        }
    }

    /// Starts a new thread to process a config generation task. Only one may
    /// be running at a given time.
    fn start_config_worker(&mut self, input_dir: &Path, output_dir: &Path) {
        // Do nothing if a worker is already processing data
        if self.worker_rx.is_some() {
            return;
        }

        self.set_message("Working...");

        let (tx_gui, rx_gui) = channel::<MessageToGUI>();

        // Keep the rx side of the channel to receive an update once the task
        // is finished
        self.worker_rx = Some(rx_gui);

        // Convert to Paths so the contents can be owned by the new thread
        let input_dir = input_dir.to_path_buf();
        let output_dir = output_dir.to_path_buf();

        // Generate the configuration on a new thread
        thread::spawn(move || {
            let result = generate_config(input_dir, output_dir);

            // Send a response to the GUI depending on what the result of the
            // operation was
            let message = match result {
                Ok(_) => MessageToGUI::Complete,
                Err(why) => MessageToGUI::Error(why),
            };

            if let Err(why) = tx_gui.send(message) {
                eprintln!("Could not send worker response back to GUI: {why}");
            }
        });
    }

    /// If a config worker is running, check on its status and update the GUI
    /// if it has finished.
    fn check_config_worker(&mut self) {
        // Only check if a worker rx channel has been created
        if let Some(rx) = &self.worker_rx {
            let message = match rx.try_recv() {
                Ok(message) => message,
                Err(err) => match err {
                    mpsc::TryRecvError::Empty => return,
                    mpsc::TryRecvError::Disconnected => {
                        eprintln!("The worker channel has disconnected.");
                        return;
                    }
                },
            };

            // Provide feedback to the user depending on the result of the
            // operation
            match message {
                MessageToGUI::Complete => {
                    self.set_message("Finished!");
                }
                MessageToGUI::Error(why) => {
                    self.set_message(&format!("Failled to generate output: {}", why.to_string()));
                }
            }

            // Remove this end of the worker channel so new workers can be
            // created
            self.worker_rx = None;
        }
    }

    /// Sets the status message which is displayed to the user
    pub fn set_message(&mut self, message: &str) {
        self.state_message = message.to_string();
        self.needs_repaint = true;
    }

    fn generate_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("Generate").clicked() {
            // Only start a config generation task if one is not already
            // running
            if self.worker_rx.is_none() {
                let mut output_dir = PathBuf::new();
                output_dir.push(&self.patch_output_folder);
                output_dir.push("aeco-patch");

                let input_dir = PathBuf::from(&self.patch_folder);
                self.start_config_worker(&input_dir, &output_dir);
            } else {
                self.set_message("Generation already in progress.")
            }
        }
    }

    fn browse_patch_folder_button(&mut self, ui: &mut egui::Ui) {
        if !ui.button("Browse").clicked() {
            return;
        }

        let file_dialog = FileDialog::new();
        let path = match file_dialog.pick_folder() {
            Some(x) => x,
            None => return,
        };

        let path_str = match path.to_str() {
            Some(x) => x,
            None => {
                self.set_message("Selected path could not be converted to a string.");
                return;
            }
        };

        self.patch_folder = path_str.to_string();
    }

    fn browse_patch_output_folder_button(&mut self, ui: &mut egui::Ui) {
        if !ui.button("Browse").clicked() {
            return;
        }

        let file_dialog = FileDialog::new();
        let path = match file_dialog.pick_folder() {
            Some(x) => x,
            None => return,
        };

        let path_str = match path.to_str() {
            Some(x) => x,
            None => {
                self.set_message("Selected path could not be converted to a string.");
                return;
            }
        };

        self.patch_output_folder = path_str.to_string();
    }
}

impl eframe::App for PatchConfigApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.check_config_worker();

        if self.needs_repaint {
            ctx.request_repaint();
            self.needs_repaint = false;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::TopBottomPanel::top("top-panel").show_inside(ui, |ui| {
                egui::SidePanel::right("generate-panel")
                    .frame(egui::Frame::none())
                    .show_inside(ui, |ui| {
                        ui.centered_and_justified(|ui| {
                            self.generate_button(ui);
                        });
                    });
                ui.label("Patch Folder");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.patch_folder);
                    self.browse_patch_folder_button(ui);
                });

                ui.label("Patch Output Folder");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.patch_output_folder);
                    self.browse_patch_output_folder_button(ui);
                });
            });
            egui::TopBottomPanel::top("message-panel").show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    egui::ScrollArea::new([true, false]).show_viewport(ui, |ui, _| {
                        ui.label(&self.state_message);
                    });
                });
            });

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show_rows(ui, 14., 50, |ui, row_range| {
                    for row in row_range {
                        // ui.label("hello");
                    }
                });
        });
    }
}

fn main() {
    let initial_window_size = Vec2::new(600., 600.);

    eframe::run_native(
        "AECO Patch Configurator",
        eframe::NativeOptions {
            // icon_data: todo!(),
            initial_window_size: Some(initial_window_size),
            resizable: false,
            ..eframe::NativeOptions::default()
        },
        Box::new(|_cc| Box::new(PatchConfigApp::new())),
    );
}