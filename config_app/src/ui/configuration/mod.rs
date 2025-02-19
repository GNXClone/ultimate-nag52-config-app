use std::{
    borrow::BorrowMut,
    sync::{Arc, Mutex}, ops::RemAssign,
};

use crate::window::PageAction;
use backend::{
    diag::Nag52Diag, ecu_diagnostics::kwp2000::{ResetType, KwpSessionType},
};
use chrono::{Datelike, Weekday};
use config_app_macros::include_base64;
use eframe::egui::Ui;
use eframe::egui::{self, *};
use egui_extras::RetainedImage;
use image::{DynamicImage, ImageFormat};
use packed_struct::PackedStructSlice;

use self::cfg_structs::{
    BoardType, DefaultProfile, EgsCanType, EngineType, IOPinConfig, MosfetPurpose, ShifterStyle,
    TcmCoreConfig, TcmEfuseConfig,
};

use super::{StatusText};

pub mod cfg_structs;

pub struct ConfigPage {
    nag: Nag52Diag,
    status: StatusText,
    scn: Option<TcmCoreConfig>,
    efuse: Option<TcmEfuseConfig>,
    show_efuse: bool,
    show_final_warning: bool,
    pcb_11_img: RetainedImage,
    pcb_12_img: RetainedImage,
    pcb_13_img: RetainedImage,
}

fn load_image(image: DynamicImage, name: &str) -> RetainedImage {
    let size = [image.width() as usize, image.height() as usize];
    let buffer = image.to_rgba8();
    let pixels = buffer.as_flat_samples();
    RetainedImage::from_color_image(
        name,
        ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
    )
}

impl ConfigPage {
    pub fn new(nag: Nag52Diag) -> Self {
        let red_img = image::load_from_memory_with_format(
            include_bytes!("../../../res/pcb_11.jpg"),
            ImageFormat::Jpeg,
        )
        .unwrap();
        let blk_img = image::load_from_memory_with_format(
            include_bytes!("../../../res/pcb_12.jpg"),
            ImageFormat::Jpeg,
        )
        .unwrap();
        let bet_img = image::load_from_memory_with_format(
            include_bytes!("../../../res/pcb_13.jpg"),
            ImageFormat::Jpeg,
        )
        .unwrap();

        let pcb_11_img = load_image(red_img, "V11-PCB");
        let pcb_12_img = load_image(blk_img, "V12-PCB");
        let pcb_13_img = load_image(bet_img, "V13-PCB");
        Self {
            nag,
            status: StatusText::Ok("".into()),
            scn: None,
            efuse: None,
            show_efuse: false,
            show_final_warning: false,
            pcb_11_img,
            pcb_12_img,
            pcb_13_img,
        }
    }
}

impl crate::window::InterfacePage for ConfigPage {
    fn make_ui(&mut self, ui: &mut Ui, frame: &eframe::Frame) -> PageAction {
        ui.heading("TCM Configuration");

        if ui.button("Read Configuration").clicked() {
            let _ = self.nag.with_kwp(|server| {
                match server.kwp_read_custom_local_identifier(0xFE) {
                    Ok(res) => {
                        match TcmCoreConfig::unpack_from_slice(&res) {
                            Ok(res) => {
                                self.status = StatusText::Ok(format!("Read OK!"));
                                self.scn = Some(res)
                            },
                            Err(_) => self.status = StatusText::Err(format!("TCM Config size is invalid. Maybe you have mismatched TCU firmware and config app version?"))
                        }
                    }
                    Err(e) => {
                        self.status =
                            StatusText::Err(format!("Error reading TCM configuration: {}", e))
                    }
                }
                match server.kwp_read_custom_local_identifier(0xFD) {
                    Ok(res) => {
                        match TcmEfuseConfig::unpack_from_slice(&res) {
                            Ok(tmp) => {
                                if tmp.board_ver == BoardType::Unknown {
                                    self.show_efuse = true;
                                }
                                self.efuse = Some(tmp);
                            },
                            Err(_) => self.status = StatusText::Err(format!("TCM EFUSE size is invalid. Maybe you have mismatched TCU firmware and config app version?"))
                        }
                    }
                    Err(e) => {
                        self.status =
                            StatusText::Err(format!("Error reading TCM EFUSE configuration: {}", e))
                    }
                }
                Ok(())
            });
        }

        let board_ver = self
            .efuse
            .clone()
            .map(|x| x.board_ver)
            .unwrap_or(BoardType::Unknown);
        if let Some(scn) = self.scn.borrow_mut() {

            ui.hyperlink_to("See getting started for more info", include_base64!("aHR0cDovL2RvY3MudWx0aW1hdGUtbmFnNTIubmV0L2VuL2dldHRpbmdzdGFydGVkI2l2ZS1yZWNlaXZlZC1hbi1hc3NlbWJsZWQtdGN1"));
            ui.hyperlink_to("See Mercedes VIN lookup table for your car configuration", include_base64!("aHR0cDovL2RvY3MudWx0aW1hdGUtbmFnNTIubmV0L2VuL2dldHRpbmdzdGFydGVkL2NvbmZpZ3VyYXRpb24vVklOTGlzdA"));

            egui::Grid::new("DGS").striped(true).show(ui, |ui| {
                let mut x = scn.is_large_nag == 1;
                ui.label("Using large 722.6");
                ui.checkbox(&mut x, "");
                scn.is_large_nag = x as u8;
                ui.end_row();

                let mut curr_profile = scn.default_profile;
                ui.label("Default drive profile");
                egui::ComboBox::from_id_source("profile")
                    .width(100.0)
                    .selected_text(format!("{:?}", curr_profile))
                    .show_ui(ui, |cb_ui| {
                        let profiles = vec![
                            DefaultProfile::Standard,
                            DefaultProfile::Comfort,
                            DefaultProfile::Winter,
                            DefaultProfile::Agility,
                            DefaultProfile::Manual,
                        ];
                        for dev in profiles {
                            cb_ui.selectable_value(
                                &mut curr_profile,
                                dev.clone(),
                                format!("{:?}", dev),
                            );
                        }
                        scn.default_profile = curr_profile
                    });
                ui.end_row();

                let mut buffer = format!("{:.2}", scn.diff_ratio as f32 / 1000.0);
                ui.label("Differential ratio");
                ui.text_edit_singleline(&mut buffer);
                if let Ok(new_ratio) = buffer.parse::<f32>() {
                    scn.diff_ratio = (new_ratio * 1000.0) as u16;
                }
                ui.end_row();

                let mut buffer = format!("{}", scn.wheel_circumference);
                ui.label("Wheel circumferance (mm)");
                ui.text_edit_singleline(&mut buffer);
                if let Ok(new_ratio) = buffer.parse::<u16>() {
                    scn.wheel_circumference = new_ratio;
                }
                ui.end_row();

                let mut engine = scn.engine_type;
                ui.label("Engine type");
                egui::ComboBox::from_id_source("engine_type")
                    .width(100.0)
                    .selected_text(format!("{:?}", engine))
                    .show_ui(ui, |cb_ui| {
                        let profiles = vec![EngineType::Diesel, EngineType::Petrol];
                        for dev in profiles {
                            cb_ui.selectable_value(&mut engine, dev.clone(), format!("{:?}", dev));
                        }
                        scn.engine_type = engine
                    });
                ui.end_row();

                let mut buffer = match scn.engine_type {
                    EngineType::Diesel => format!("{}", scn.red_line_dieselrpm),
                    EngineType::Petrol => format!("{}", scn.red_line_petrolrpm),
                };
                ui.label("Engine redline RPM");
                ui.text_edit_singleline(&mut buffer);
                if let Ok(rpm) = buffer.parse::<u16>() {
                    match scn.engine_type {
                        EngineType::Diesel => scn.red_line_dieselrpm = rpm,
                        EngineType::Petrol => scn.red_line_petrolrpm = rpm,
                    }
                }
                ui.end_row();

                let mut x = scn.is_four_matic == 1;
                ui.label("Four matic");
                ui.checkbox(&mut x, "");
                scn.is_four_matic = (x as u8);
                ui.end_row();

                if scn.is_four_matic == 1 {
                    let mut buffer =
                        format!("{:.2}", scn.transfer_case_high_ratio as f32 / 1000.0);
                    ui.label("Transfer case high ratio");
                    ui.text_edit_singleline(&mut buffer);
                    if let Ok(new_ratio) = buffer.parse::<f32>() {
                        scn.transfer_case_high_ratio = (new_ratio * 1000.0) as u16;
                    }
                    ui.end_row();

                    let mut buffer =
                        format!("{:.2}", scn.transfer_case_low_ratio as f32 / 1000.0);
                    ui.label("Transfer case low ratio");
                    ui.text_edit_singleline(&mut buffer);
                    if let Ok(new_ratio) = buffer.parse::<f32>() {
                        scn.transfer_case_low_ratio = (new_ratio * 1000.0) as u16;
                    }
                    ui.end_row();
                }

                let mut buffer =
                        format!("{:.1}", scn.engine_drag_torque as f32 / 10.0);
                    ui.label("Engine drag torque");
                    ui.text_edit_singleline(&mut buffer);
                    if let Ok(drg) = buffer.parse::<f32>() {
                        scn.engine_drag_torque = (drg * 10.0) as u16;
                    }
                    ui.end_row();

                ui.label("EGS CAN Layer: ");
                let mut can = scn.egs_can_type;
                egui::ComboBox::from_id_source("can_layer")
                    .width(100.0)
                    .selected_text(format!("{:?}", can))
                    .show_ui(ui, |cb_ui| {
                        let layers = match board_ver {
                            BoardType::Unknown | BoardType::V11 => {
                                vec![EgsCanType::UNKNOWN, EgsCanType::EGS52, EgsCanType::EGS53]
                            }
                            _ => vec![
                                EgsCanType::UNKNOWN,
                                EgsCanType::EGS51,
                                EgsCanType::EGS52,
                                EgsCanType::EGS53,
                            ],
                        };
                        for layer in layers {
                            cb_ui.selectable_value(&mut can, layer.clone(), format!("{:?}", layer));
                        }
                        scn.egs_can_type = can
                    });
                ui.end_row();

                if board_ver == BoardType::V12 || board_ver == BoardType::V13 {
                    // 1.2 or 1.3 config
                    ui.label("Shifter style: ");
                    let mut ss = scn.shifter_style;
                    egui::ComboBox::from_id_source("shifter_style")
                        .width(200.0)
                        .selected_text(format!("{:?}", ss))
                        .show_ui(ui, |cb_ui| {
                            let options = vec![
                                ShifterStyle::EWM_CAN,
                                ShifterStyle::TRRS,
                                ShifterStyle::SLR_MCLAREN,
                            ];
                            for o in options {
                                cb_ui.selectable_value(&mut ss, o.clone(), format!("{:?}", o));
                            }
                            scn.shifter_style = ss
                        });
                    ui.end_row();
                }

                if board_ver == BoardType::V13 {
                    // Only v1.3 config
                    ui.label("GPIO usage: ");
                    let mut ss = scn.io_0_usage;
                    egui::ComboBox::from_id_source("gpio_usage")
                        .width(200.0)
                        .selected_text(format!("{:?}", ss))
                        .show_ui(ui, |cb_ui| {
                            let options = vec![
                                IOPinConfig::NotConnected,
                                IOPinConfig::Input,
                                IOPinConfig::Output,
                            ];
                            for o in options {
                                cb_ui.selectable_value(&mut ss, o.clone(), format!("{:?}", o));
                            }
                            scn.io_0_usage = ss
                        });
                    ui.end_row();

                    if scn.io_0_usage == IOPinConfig::Input {
                        let mut t = format!("{}", scn.input_sensor_pulses_per_rev);
                        ui.label("Input sensor pulses/rev");
                        ui.text_edit_singleline(&mut t);
                        if let Ok(prev) = t.parse::<u8>() {
                            scn.input_sensor_pulses_per_rev = prev;
                        }
                        ui.end_row();
                    } else if scn.io_0_usage == IOPinConfig::Output {
                        let mut t = format!("{}", scn.output_pulse_width_per_kmh);
                        ui.label("Pulse width (us) per kmh");
                        ui.text_edit_singleline(&mut t);
                        if let Ok(prev) = t.parse::<u8>() {
                            scn.output_pulse_width_per_kmh = prev;
                        }
                        ui.end_row();
                    }
                    ui.label("General MOSFET usage: ");
                    let mut ss = scn.mosfet_purpose;
                    egui::ComboBox::from_id_source("mosfet_purpose")
                        .width(200.0)
                        .selected_text(format!("{:?}", ss))
                        .show_ui(ui, |cb_ui| {
                            let options = vec![
                                MosfetPurpose::NotConnected,
                                MosfetPurpose::TorqueCutTrigger,
                                MosfetPurpose::B3BrakeSolenoid,
                            ];
                            for o in options {
                                cb_ui.selectable_value(&mut ss, o.clone(), format!("{:?}", o));
                            }
                            scn.mosfet_purpose = ss
                        });
                    ui.end_row();
                }
            });

            if ui.button("Write SCN configuration").clicked() {
                let res = {
                    let mut x: Vec<u8> = vec![0x3B, 0xFE];
                    x.extend_from_slice(&scn.clone().pack_to_vec().unwrap());
                    self.nag.with_kwp(|server| {
                        server.kwp_set_session(KwpSessionType::Reprogramming.into())?;
                        server.send_byte_array_with_response(&x)?;
                        server.kwp_reset_ecu(ResetType::PowerOnReset.into())?;
                        Ok(())
                    })
                };
            }
        }

        if let Some(efuse) = self.efuse.borrow_mut() {
            if self.show_efuse {
                ui.heading("EFUSE CONFIG");
                ui.label("IMPORTANT! This can only be set once! Be careful!");
                ui.spacing();
                ui.horizontal(|row| {
                    row.vertical(|col| {
                        col.label("V1.1 - Red PCB (12/12/21)");
                        col.image(
                            self.pcb_11_img.texture_id(col.ctx()),
                            Vec2::from((200.0, 150.0)),
                        );
                    });
                    row.separator();
                    row.vertical(|col| {
                        col.label("V1.2 - Black PCB (07/07/22) with TRRS support");
                        col.image(
                            self.pcb_12_img.texture_id(col.ctx()),
                            Vec2::from((200.0, 150.0)),
                        );
                    });
                    row.separator();
                    row.vertical(|col| {
                        col.label("V1.3 - Black PCB (12/12/22) - Latest PCB");
                        col.image(
                            self.pcb_13_img.texture_id(col.ctx()),
                            Vec2::from((230.0, 150.0)),
                        );
                    });
                });
                let mut ver = efuse.board_ver;
                ui.label("Choose board variant: ");
                egui::ComboBox::from_id_source("board_ver")
                    .width(100.0)
                    .selected_text(format!("{:?}", efuse.board_ver))
                    .show_ui(ui, |cb_ui| {
                        let profiles = vec![BoardType::V11, BoardType::V12, BoardType::V13];
                        for (pos, dev) in profiles.iter().enumerate() {
                            cb_ui.selectable_value(&mut ver, dev.clone(), dev.to_string());
                        }
                        efuse.board_ver = ver
                    });
            }
            if self.show_efuse && efuse.board_ver != BoardType::Unknown {
                if ui.button("Write EFUSE configuration").clicked() {
                    self.show_final_warning = true;
                }
            }
        }

        let mut tmp = self.show_final_warning;

        let ss = ui.ctx().input(|x| x.screen_rect());
        let mut reload = false;
        egui::Window::new("ARE YOU SURE?")
            .open(&mut self.show_final_warning)
            .fixed_pos(Pos2::new(ss.size().x / 2.0, ss.size().y / 2.0))
            .show(ui.ctx(), |win| {
                win.label("EFUSE CONFIGURATION CANNOT BE UN-DONE");
                win.label(
                    "Please double check and ensure you have selected the right board variant!",
                );
                win.horizontal(|row| {
                    if row.button("Take me back").clicked() {
                        tmp = false;
                    }
                    if row.button("Yes, I am sure!").clicked() {
                        let mut efuse = self.efuse.clone().unwrap();
                        let date = chrono::Utc::now().date_naive();
                        efuse.manf_day = date.day() as u8;
                        efuse.manf_week = date.iso_week().week() as u8;
                        efuse.manf_month = date.month() as u8;
                        efuse.manf_year = (date.year() - 2000) as u8;
                        println!("EFUSE: {:?}", efuse);

                        let mut x = vec![0x3Bu8, 0xFD];
                        x.extend_from_slice(&efuse.pack_to_vec().unwrap());
                        self.nag.with_kwp(|server| {
                            server.kwp_set_session(KwpSessionType::Reprogramming.into())?;
                            server.send_byte_array_with_response(&x)?;
                            server.kwp_reset_ecu(ResetType::PowerOnReset.into())?;
                            Ok(())
                        });
                        tmp = false;
                    }
                })
            });
        if reload {
            *self = Self::new(self.nag.clone());
        }
        self.show_final_warning = tmp;

        ui.add(self.status.clone());
        PageAction::None
    }

    fn get_title(&self) -> &'static str {
        "Configuration"
    }

    fn should_show_statusbar(&self) -> bool {
        true
    }
}
