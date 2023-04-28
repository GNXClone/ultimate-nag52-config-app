use backend::diag::ident::IdentData;
use backend::diag::Nag52Diag;
use backend::diag::settings::LinearInterpSettings;
use backend::diag::settings::TcuSettings;
use backend::diag::settings::unpack_settings;
use backend::serde_yaml;
use backend::serde_yaml::Mapping;
use backend::serde_yaml::Value;
use config_app_macros::include_base64;
use eframe::egui;
use eframe::Frame;
use eframe::egui::CollapsingHeader;
use eframe::egui::DragValue;
use eframe::egui::RichText;
use eframe::egui::ScrollArea;
use eframe::egui::plot::Line;
use eframe::egui::plot::Plot;
use eframe::egui::plot::PlotPoints;
use eframe::epaint::Color32;
use serde_json::Number;
use std::borrow::BorrowMut;
use std::ops::RangeInclusive;
use std::sync::{mpsc, Arc, Mutex};
use crate::window::{InterfacePage, PageAction};

use super::settings_ui_gen::TcuAdvSettingsUi;
use super::updater::UpdatePage;
use super::widgets::number_input::NumberInputWidget;
use super::{
    configuration::ConfigPage,
    diagnostics::solenoids::SolenoidPage,
    io_maipulator::IoManipulatorPage, map_editor::MapEditor, routine_tests::RoutinePage,
};
use crate::ui::diagnostics::DiagnosticsPage;

pub struct MainPage {
    show_about_ui: bool,
    diag_server: &'static mut Nag52Diag,
    info: Option<IdentData>,
    sn: Option<String>,
    first_run: bool,
    cell_memory: Option<String>,
}

impl MainPage {
    pub fn new(nag: Nag52Diag) -> Self {
        // Static mutable ref creation
        // this Nag52 lives the whole lifetime of the app once created,
        // so we have no need to clone it constantly, just throw the pointer around at
        // the subpages.
        //
        // We can keep it here as a ref to create a box from it when Drop() is called
        // so we can drop it safely without a memory leak
        let static_ref: &'static mut Nag52Diag = Box::leak(Box::new(nag));
        
        Self {
            show_about_ui: false,
            diag_server: static_ref,
            info: None,
            sn: None,
            first_run: false,
            cell_memory: None
        }
    }
}

impl InterfacePage for MainPage {
    fn make_ui(&mut self, ui: &mut egui::Ui, frame: &Frame) -> crate::window::PageAction {
        if !self.first_run {
            self.first_run = true;
            return PageAction::RegisterNag(self.diag_server.clone());
        }
        ui.vertical_centered(|x| {
            x.heading("Welcome to the Ultimate-NAG52 configuration app!");
            if env!("GIT_BUILD").ends_with("-dirty") {
                x.label(format!("Config app version {} (Build {})", env!("CARGO_PKG_VERSION"), env!("GIT_BUILD")));
                x.label(RichText::new("Warning. You have a modified copy of the config app! Bugs may be present!").color(Color32::RED));
            } else {
                x.label(format!("Config app version {} (Build {})", env!("CARGO_PKG_VERSION"), env!("GIT_BUILD")));
            }
        });
        ui.separator();
        ui.label(r#"
            This application lets you do many things with the TCU!
            If you are lost or need help, you can always consult the wiki below,
            or join the Ultimate-NAG52 discussions Telegram group!
        "#);
        ui.heading("Useful links");
        // Weblinks are base64 encoded to avoid potential scraping
        ui.hyperlink_to(format!("📓 Ultimate-NAG52 wiki"), include_base64!("ZG9jcy51bHRpbWF0ZS1uYWc1Mi5uZXQ"));
        ui.hyperlink_to(format!("💁 Ultimate-NAG52 dicsussion group"), include_base64!("aHR0cHM6Ly90Lm1lLyt3dU5wZkhua0tTQmpNV0pr"));
        ui.hyperlink_to(format!(" Project progress playlist"), include_base64!("aHR0cHM6Ly93d3cueW91dHViZS5jb20vcGxheWxpc3Q_bGlzdD1QTHhydy00VnQ3eHR1OWQ4bENrTUNHMF9LN29IY3NTTXRG"));
        ui.label("Code repositories");
        ui.hyperlink_to(format!(" The configuration app"), include_base64!("aHR0cHM6Ly9naXRodWIuY29tL3JuZC1hc2gvdWx0aW1hdGUtbmFnNTItY29uZmlnLWFwcA"));
        ui.hyperlink_to(format!(" TCU Firmware"), include_base64!("aHR0cDovL2dpdGh1Yi5jb20vcm5kLWFzaC91bHRpbWF0ZS1uYWc1Mi1mdw"));
        ui.add(egui::Separator::default());
        let mut create_page = None;
        ui.vertical_centered(|v| {
            v.heading("Tools");
            if v.button("Updater").clicked() {
                create_page = Some(PageAction::Add(Box::new(UpdatePage::new(
                    self.diag_server.clone(),
                ))));
            }
            if v.button("Diagnostics").clicked() {
                create_page = Some(PageAction::Add(Box::new(DiagnosticsPage::new(
                    self.diag_server.clone(),
                ))));
            }
            if v.button("Solenoid live view").clicked() {
                create_page = Some(PageAction::Add(Box::new(SolenoidPage::new(
                    self.diag_server.clone(),
                ))));
            }
            if v.button("IO Manipulator").clicked() {
                create_page = Some(PageAction::Add(Box::new(IoManipulatorPage::new(
                    self.diag_server.clone(),
                ))));
            }
            if v.button("Diagnostic routine executor").clicked() {
                create_page = Some(PageAction::Add(Box::new(RoutinePage::new(
                    self.diag_server.clone(),
                ))));
            }
            if v.button("Map Tuner").clicked() {
                create_page = Some(PageAction::Add(Box::new(MapEditor::new(
                    self.diag_server.clone(),
                ))));
            }
            if v.button("TCU Program settings").on_hover_text("CAUTION. DANGEROUS!").clicked() {
                create_page = Some(PageAction::Add(Box::new(TcuAdvSettingsUi::new(
                    self.diag_server.clone(),
                ))));
            }
            if v.button("Configure drive profiles").clicked() {
                create_page = Some(
                    PageAction::SendNotification { 
                        text: "You have found a unimplemented feature!".into(), 
                        kind: egui_toast::ToastKind::Info 
                    }
                );
            }
            if v.button("Configure vehicle / gearbox").clicked() {
                create_page = Some(PageAction::Add(Box::new(ConfigPage::new(
                    self.diag_server.clone(),
                ))));
            }
        });

        if let Some(page) = create_page {
            return page;
        }

        if self.show_about_ui {
            egui::containers::Window::new("About")
                .resizable(false)
                .collapsible(false)
                .default_pos(&[400f32, 300f32])
                .show(ui.ctx(), |win| {
                    win.vertical(|about_cols| {
                        about_cols.heading("Version data");
                        about_cols.label(format!(
                            "Configuration app version: {}",
                            env!("CARGO_PKG_VERSION")
                        ));
                        about_cols.separator();
                        if let Some(ident) = self.info {
                            about_cols.heading("TCU Data");
                            about_cols.label(format!(
                                "ECU Serial number: {}",
                                self.sn.clone().unwrap_or("Unknown".into())
                            ));
                            about_cols.label(format!(
                                "PCB Version: {} (HW date: {} week 20{})",
                                ident.board_ver, ident.hw_week, ident.hw_year
                            ));
                            about_cols.label(format!(
                                "PCB Production date: {}/{}/20{}",
                                ident.manf_day, ident.manf_month, ident.manf_year
                            ));
                            about_cols.label(format!(
                                "PCB Software date: week {} of 20{}",
                                ident.sw_week, ident.sw_year
                            ));
                            about_cols
                                .label(format!("EGS CAN Matrix selected: {}", ident.egs_mode));
                        } else {
                            about_cols.heading("Could not read TCU Ident data");
                        }

                        about_cols.separator();
                        about_cols.heading("Open source");
                        about_cols.add(egui::Hyperlink::from_label_and_url(
                            "Github repository (Configuration utility)",
                            "https://github.com/rnd-ash/ultimate-nag52-config-app",
                        ));
                        about_cols.add(egui::Hyperlink::from_label_and_url(
                            "Github repository (TCM source code)",
                            "https://github.com/rnd-ash/ultimate-nag52-fw",
                        ));
                        about_cols.separator();
                        about_cols.heading("Author");
                        about_cols.add(egui::Hyperlink::from_label_and_url(
                            "rnd-ash",
                            "https://github.com/rnd-ash",
                        ));
                        if about_cols.button("Close").clicked() {
                            self.show_about_ui = false;
                        }
                    })
                });
        }

        PageAction::None
    }

    fn get_title(&self) -> &'static str {
        "Ultimate-Nag52 configuration utility (Home)"
    }

    fn should_show_statusbar(&self) -> bool {
        true
    }

    fn destroy_nag(&self) -> bool {
        true
    }

}

impl Drop for MainPage {
    fn drop(&mut self) {
        // Create a temp box so we can drop it
        let b = unsafe { Box::from_raw(self.diag_server) };
        drop(b);
    }
}
