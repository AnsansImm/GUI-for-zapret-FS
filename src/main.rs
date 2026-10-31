#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod backend;
mod browser_sites;
mod editor;
mod fakes;
mod ipset;
mod list_store;
mod sqlite_history;
mod testing;
mod updates;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::mpsc};
#[derive(Default, Serialize, Deserialize)]
struct Config {
    root: Option<PathBuf>,
}
struct App {
    logo: egui::TextureHandle,
    config: Config,
    config_path: PathBuf,
    tab: usize,
    strategy: String,
    log: String,
    rx: Option<mpsc::Receiver<String>>,
    files: Vec<PathBuf>,
    selected: Option<PathBuf>,
    text: String,
    disk: String,
    mode: String,
    tcp: String,
    udp: String,
    confirm: Option<String>,
    last_edit: Option<std::time::Instant>,
    cursor_to_end: bool,
    report: Vec<backend::Check>,
    diagnostic_pending: bool,
    repair_pending: bool,
    fake_confirm: Option<(String, String)>,
    tests: testing::Tests,
    updates: updates::Updates,
    sites: browser_sites::Sites,
    snapshot: Option<backend::Snapshot>,
    snapshot_rx: Option<mpsc::Receiver<Result<backend::Snapshot, String>>>,
    snapshot_time: Option<std::time::Instant>,
    snapshot_error: String,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10., 10.);
        style.spacing.button_padding = egui::vec2(14., 9.);
        style.visuals.panel_fill = egui::Color32::from_rgb(29, 33, 40);
        style.visuals.window_fill = egui::Color32::from_rgb(36, 42, 51);
        style.visuals.extreme_bg_color = egui::Color32::from_rgb(26, 32, 40);
        style.visuals.faint_bg_color = egui::Color32::from_rgb(38, 46, 57);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(42, 65, 94);
        style.visuals.selection.stroke.color = egui::Color32::from_rgb(193, 222, 255);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(43, 52, 66);
        style.visuals.widgets.inactive.rounding = egui::Rounding::same(7.);
        style.visuals.widgets.hovered.rounding = egui::Rounding::same(7.);
        style.visuals.widgets.active.rounding = egui::Rounding::same(7.);
        style.visuals.window_rounding = egui::Rounding::same(10.);
        cc.egui_ctx.set_style(style);
        let config_path = std::env::current_exe()
            .unwrap()
            .with_file_name("config.json");
        let mut config: Config = fs::read(&config_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if config.root.is_none() {
            if let Ok(dir) = std::env::current_dir() {
                config.root = fs::read_dir(dir)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .find(|p| p.join("service.bat").is_file());
            }
        }
        let mut a = Self {
            logo: {
                let icon = eframe::icon_data::from_png_bytes(include_bytes!("../logo1.png"))
                    .expect("Invalid application logo");
                cc.egui_ctx.load_texture(
                    "app-logo",
                    egui::ColorImage::from_rgba_unmultiplied(
                        [icon.width as usize, icon.height as usize],
                        &icon.rgba,
                    ),
                    egui::TextureOptions::LINEAR,
                )
            },
            config,
            config_path,
            tab: 0,
            strategy: String::new(),
            log: "Выберите папку Zapret. Системные действия выполняются только по кнопке.".into(),
            rx: None,
            files: vec![],
            selected: None,
            text: String::new(),
            disk: String::new(),
            mode: "disabled".into(),
            tcp: "1024-65535".into(),
            udp: "1024-65535".into(),
            confirm: None,
            last_edit: None,
            cursor_to_end: false,
            report: Vec::new(),
            diagnostic_pending: false,
            repair_pending: false,
            fake_confirm: None,
            tests: testing::Tests::new(),
            updates: updates::Updates::default(),
            sites: browser_sites::Sites::default(),
            snapshot: None,
            snapshot_rx: None,
            snapshot_time: None,
            snapshot_error: String::new(),
        };
        a.load();
        a
    }
    fn save_editor(&mut self) -> bool {
        if self.last_edit.is_none() {
            return true;
        }
        let Some(path) = &self.selected else {
            return true;
        };
        let content = editor::normalize(&self.text, editor::is_ip(path));
        if content == self.disk {
            self.last_edit = None;
            return true;
        }
        let lists = path.parent().unwrap();
        let _guard = match list_store::lock(lists) {
            Ok(g) => g,
            Err(e) => {
                self.log = e;
                return false;
            }
        };
        if let Err(e) = list_store::recover(lists) {
            self.log = e;
            return false;
        }
        match fs::read_to_string(path) {
            Ok(current) if current == self.disk => {
                match list_store::atomic(path, content.as_bytes()) {
                    Ok(()) => {
                        self.disk = content;
                        self.last_edit = None;
                        self.log = "Изменения сохранены".into();
                        true
                    }
                    Err(e) => {
                        self.log = format!("Ошибка сохранения: {e}");
                        false
                    }
                }
            }
            _ => {
                self.log = "Файл изменён извне или недоступен. Изменения остаются в редакторе; переход и закрытие отменены. Скопируйте текст перед повторным открытием файла.".into();
                false
            }
        }
    }
    fn open_list(&mut self, path: PathBuf) {
        if !self.save_editor() {
            return;
        }
        let _guard = match list_store::lock(path.parent().unwrap()) {
            Ok(g) => g,
            Err(e) => {
                self.log = e;
                return;
            }
        };
        if let Err(e) = list_store::recover(path.parent().unwrap()) {
            self.log = e;
            return;
        }
        match fs::read_to_string(&path) {
            Ok(text) => {
                self.text = editor::editable(&text, editor::is_ip(&path));
                self.disk = text;
                self.selected = Some(path);
                self.last_edit = None;
                self.cursor_to_end = true;
            }
            Err(e) => self.log = e.to_string(),
        }
    }
    fn editor_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        page_title(ui, "Редактор списков");
        settings_card(ui, |ui| {
            let Some(path) = self.selected.clone() else {
                ui.label("Выберите лист в колонке слева");
                return;
            };
            let ip = editor::is_ip(&path);
            ui.label(path.file_name().unwrap().to_string_lossy());
            ui.label(if ip {
                "IPSet: адреса IPv4/IPv6 и подсети (/). Пробел — новая строка; # — комментарий."
            } else {
                "Домены: a–z, 0–9, -, ., _, *. Пробел — новая строка; # — комментарий."
            });
            ui.weak("Автосохранение через 400 мс после ввода");
            let id = ui.make_persistent_id(("list_editor", &path));
            let output = egui::ScrollArea::vertical()
                .id_salt(&path)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::TextEdit::multiline(&mut self.text)
                        .id(id)
                        .desired_width(f32::INFINITY)
                        .desired_rows(22)
                        .font(egui::TextStyle::Monospace)
                        .show(ui)
                })
                .inner;
            if self.cursor_to_end {
                let mut state = output.state.clone();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(self.text.chars().count()),
                    )));
                state.store(ctx, id);
                output.response.request_focus();
                self.cursor_to_end = false;
                ctx.request_repaint();
            }
            if output.response.changed() {
                // Translate cursor positions through filtering, including Unicode comments.
                let range = output.cursor_range.map(|r| r.as_ccursor_range());
                let raw = self.text.clone();
                self.text = editor::editable(&raw, ip);
                if let Some(mut range) = range {
                    let map = |index: usize| {
                        editor::normalize(&raw.chars().take(index).collect::<String>(), ip)
                            .chars()
                            .count()
                            + usize::from(
                                index > 0
                                    && raw.chars().nth(index - 1).is_some_and(char::is_whitespace)
                                    && !raw
                                        .lines()
                                        .last()
                                        .unwrap_or("")
                                        .trim_start()
                                        .starts_with('#'),
                            )
                    };
                    range.primary.index = map(range.primary.index).min(self.text.chars().count());
                    range.secondary.index =
                        map(range.secondary.index).min(self.text.chars().count());
                    let mut state = output.state;
                    state.cursor.set_char_range(Some(range));
                    state.store(ctx, id);
                }
                self.last_edit = Some(std::time::Instant::now());
                self.log = "Изменения будут сохранены…".into();
            }
        });
    }
    fn load(&mut self) {
        self.report.clear();
        self.fake_confirm = None;
        if let Some(root) = &self.config.root {
            self.strategy = backend::strategies(root)
                .first()
                .cloned()
                .unwrap_or_default();
            self.files = fs::read_dir(root.join("lists"))
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|s| s == "txt"))
                .collect();
            self.files.sort_by_key(|p| {
                (
                    !p.file_name().unwrap().to_string_lossy().contains("user"),
                    p.clone(),
                )
            });
            let g = fs::read_to_string(root.join("utils/game_filter.enabled")).unwrap_or_default();
            self.mode = "disabled".into();
            for l in g.lines() {
                if let Some((k, v)) = l.split_once('=') {
                    match k {
                        "mode" => self.mode = v.into(),
                        "tcp" => self.tcp = v.into(),
                        "udp" => self.udp = v.into(),
                        _ => {}
                    }
                }
            }
        }
    }
    fn workspace(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        match self.tab {
            0 => {
                self.management_ui(ui);
            }
            1 => {
                page_title(ui, "Фильтры");
                settings_card(ui, |ui| {
                    section_title(ui, "Game Filter");
                    ui.horizontal_wrapped(|ui| {
                        for (value, label) in [
                            ("disabled", "Отключён"),
                            ("all", "TCP и UDP"),
                            ("tcp", "TCP"),
                            ("udp", "UDP"),
                        ] {
                            let fill = if self.mode == value {
                                ui.visuals().selection.bg_fill
                            } else {
                                ui.visuals().widgets.inactive.weak_bg_fill
                            };
                            if ui.add(egui::Button::new(label).fill(fill)).clicked() {
                                self.mode = value.into();
                            }
                        }
                    });
                    ui.label("TCP: порты и диапазоны через запятую");
                    ui.text_edit_singleline(&mut self.tcp);
                    ui.label("UDP");
                    ui.text_edit_singleline(&mut self.udp);
                    if ui.button("Сохранить настройки").clicked() {
                        if backend::valid_ports(&self.tcp) && backend::valid_ports(&self.udp) {
                            let p = self
                                .config
                                .root
                                .as_ref()
                                .unwrap()
                                .join("utils/game_filter.enabled");
                            self.log=match fs::write(p,format!("mode={}\ntcp={}\nudp={}\n",self.mode,self.tcp,self.udp)){Ok(_)=>"Сохранено. Для службы повторно примените выбранную стратегию на вкладке «Управление».".into(),Err(e)=>e.to_string()};
                        } else {
                            self.log =
                            "Проверьте порты: допустимы значения 1–65535 и возрастающие диапазоны."
                                .into()
                        }
                    }
                });
                self.ipset_ui(ui);
            }
            2 => {
                self.editor_ui(ui, ctx);
            }
            3 => {
                self.tests_ui(ui);
            }
            4 => {
                page_title(ui, "Диагностика");
                settings_card(ui, |ui| {
                    ui.weak("Проверка не меняет настройки Windows и файлы Zapret. Предупреждения указывают на возможную причину, а не подтверждают неисправность.");
                    ui.horizontal(|ui| {
                        if ui.button("Запустить проверку").clicked() {
                            self.launch("diagnostics".into());
                        }
                        if ui
                            .add_enabled(
                                !self.report.is_empty(),
                                egui::Button::new("Копировать отчёт"),
                            )
                            .clicked()
                        {
                            ui.ctx().copy_text(
                                self.report
                                    .iter()
                                    .map(|c| format!("[{}] {}: {}", c.level, c.title, c.detail))
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            );
                        }
                    });
                    ui.separator();
                    if self.report.is_empty() {
                        ui.weak("Проверка ещё не запускалась.");
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for check in &self.report {
                            let (label, color) = match check.level.as_str() {
                                "ok" => ("В порядке", egui::Color32::from_rgb(115, 200, 150)),
                                "warn" => ("Внимание", egui::Color32::from_rgb(235, 190, 100)),
                                "unknown" => {
                                    ("Не проверено", egui::Color32::from_rgb(230, 140, 140))
                                }
                                _ => ("Информация", egui::Color32::from_rgb(140, 185, 230)),
                            };
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong(&check.title);
                                    ui.colored_label(color, label);
                                });
                                ui.label(&check.detail);
                                if check.title == "Базовая фильтрация Windows"
                                    && check.level == "warn"
                                    && ui.button("Запустить BFE…").clicked()
                                {
                                    self.confirm = Some("fix_bfe".into());
                                }
                                if check.title == "TCP timestamps"
                                    && ui.button("Включить TCP timestamps…").clicked()
                                {
                                    self.confirm = Some("fix_timestamps".into());
                                }
                            });
                        }
                    });
                });
            }
            5 => {
                self.fakes_ui(ui);
            }
            6 => {
                self.updates_ui(ui);
            }
            7 => self.sites_ui(ui),
            _ => {
                self.management_ui(ui);
            }
        }
    }
    fn refresh_snapshot(&mut self) {
        if self.snapshot_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.snapshot_rx = Some(rx);
        self.snapshot_time = Some(std::time::Instant::now());
        std::thread::spawn(move || {
            let _ = tx.send(backend::snapshot());
        });
    }
    fn status_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.image((self.logo.id(), egui::vec2(32., 32.)));
            ui.heading("GUI-for-zapret-FS");
            ui.add_space(18.);
            let (text, color) = match &self.snapshot {
                Some(s) if s.service_running && s.standalone => (
                    "● Работают служба и отдельный батник",
                    egui::Color32::LIGHT_RED,
                ),
                Some(s) if s.service_running => (
                    "● Работает служба Zapret",
                    egui::Color32::from_rgb(155, 223, 177),
                ),
                Some(s) if s.winws => (
                    "● Zapret запущен отдельно",
                    egui::Color32::from_rgb(155, 223, 177),
                ),
                Some(_) => (
                    "● Zapret остановлен",
                    egui::Color32::from_rgb(229, 188, 120),
                ),
                None => ("● Состояние уточняется", egui::Color32::GRAY),
            };
            ui.colored_label(color, text);
            if let Some(s) = &self.snapshot {
                if !s.strategy.is_empty() {
                    ui.weak(&s.strategy);
                }
            }
            if ui
                .add_enabled(
                    self.snapshot_rx.is_none(),
                    egui::Button::new("Обновить состояние"),
                )
                .clicked()
            {
                self.refresh_snapshot();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let available = self.rx.is_none()
                    && self.updates.install_rx.is_none()
                    && !self.tests.busy()
                    && !self.tests.needs_recovery();
                if ui
                    .add_enabled(available, egui::Button::new("Выбрать папку…"))
                    .clicked()
                {
                    self.pick_folder();
                }
                let path = self
                    .config
                    .root
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Папка Zapret не выбрана".into());
                let width = ui.available_width().min(300.).max(0.);
                ui.add_sized(
                    [width, 20.],
                    egui::Label::new(egui::RichText::new(&path).weak()).truncate(),
                )
                .on_hover_text(&path);
            });
        });
        if self.updates.install_rx.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Обновление Zapret: загрузка и перенос списков…");
            });
        }
        if !self.snapshot_error.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_RED, &self.snapshot_error);
        }
    }
    fn management_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.);
        page_title(ui, "Управление Zapret");
        ui.add_space(12.);
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(16.))
            .rounding(10.)
            .fill(egui::Color32::from_rgb(38, 46, 57))
            .show(ui, |ui| {
                ui.set_min_width((ui.available_width() - 8.).max(0.));
                ui.label("Выбранная стратегия");
                ui.strong(&self.strategy);
                let installed = self
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| format!("{}.bat", s.strategy) == self.strategy);
                if installed {
                    ui.colored_label(
                        egui::Color32::from_rgb(155, 223, 177),
                        "Сохранена в службе (это не статус запуска)",
                    );
                }
            });
        ui.add_space(12.);
        let standalone = self.snapshot.as_ref().is_some_and(|s| s.standalone);
        let service = self.snapshot.as_ref().is_some_and(|s| s.service_exists);
        let running = self.snapshot.as_ref().is_some_and(|s| s.service_running);
        let ready = self.snapshot.is_some() && self.snapshot_error.is_empty() && self.rx.is_none();
        settings_card(ui, |ui| {
            section_title(ui, "Отдельный запуск батником");
            ui.label(if self.snapshot.is_none() {
                "Состояние уточняется"
            } else if standalone {
                "Работает: отдельный winws"
            } else {
                "Остановлен"
            });
            ui.weak("Выбранный .bat, без установки службы и автозапуска Windows.");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        ready && !standalone && !running,
                        egui::Button::new("Запустить выбранный батник"),
                    )
                    .clicked()
                {
                    self.launch("start_standalone".into());
                }
            });
            ui.weak("Для остановки закройте окно запущенного батника крестиком.");
        });
        ui.add_space(10.);
        settings_card(ui, |ui| {
            section_title(ui, "Служба Windows — zapret");
            ui.label(if self.snapshot.is_none() {
                "Состояние уточняется"
            } else if running {
                "Работает"
            } else if service {
                "Установлена, остановлена"
            } else {
                "Не установлена"
            });
            if let Some(s) = &self.snapshot {
                if service {
                    ui.label(format!("Стратегия службы: {}", s.strategy));
                }
            }
            ui.weak("Запуск службы использует сохранённую стратегию. Установка включает автозапуск с Windows.");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        ready && !standalone,
                        egui::Button::new(if service {
                            "Применить стратегию и перезапустить службу…"
                        } else {
                            "Установить и запустить службу…"
                        }),
                    )
                    .clicked()
                {
                    self.confirm = Some("install".into());
                }
                if ui
                    .add_enabled(
                        ready && service && !running && !standalone,
                        egui::Button::new("Запустить службу"),
                    )
                    .clicked()
                {
                    self.launch("start".into());
                }
                if ui
                    .add_enabled(ready && running, egui::Button::new("Остановить службу"))
                    .clicked()
                {
                    self.launch("stop".into());
                }
            });
        });
        if standalone {
            ui.colored_label(
                egui::Color32::YELLOW,
                "Перед запуском службы остановите отдельный запуск.",
            );
        }
        if ui.button("Подробное состояние").clicked() {
            self.launch("status".into());
        }
        ui.add_space(12.);
        ui.add_space(10.);
        if ui
            .add(egui::Button::new(
                egui::RichText::new("Удалить службу…")
                    .color(egui::Color32::from_rgb(216, 166, 166)),
            ))
            .clicked()
        {
            self.confirm = Some("remove".into());
        }
        ui.weak("Установка включает TCP timestamps. Удаление службы сохраняет файлы и списки.");
    }
    fn pick_folder(&mut self) {
        if let Some(p) = rfd::FileDialog::new().pick_folder() {
            if !p.join("service.bat").is_file() || !p.join("bin/winws.exe").is_file() {
                self.log = "В папке не найдены service.bat и bin/winws.exe".into();
                return;
            }
            if !self.save_editor() {
                return;
            }
            self.config.root = Some(p);
            self.selected = None;
            self.load();
            self.log = match fs::write(
                &self.config_path,
                serde_json::to_vec_pretty(&self.config).unwrap(),
            ) {
                Ok(_) => "Папка сохранена".into(),
                Err(e) => e.to_string(),
            };
        }
    }
    fn sites_ui(&mut self, ui: &mut egui::Ui) {
        page_title(ui, "Добавить сайт из браузера");
        ui.weak("Выберите сайт из вкладок браузера или истории либо введите адрес вручную.");
        ui.columns(2, |columns| {
            settings_card(&mut columns[0], |ui| {
                section_title(ui, "Браузер и источник");
        ui.add_enabled_ui(self.sites.rx.is_none(), |ui| {
            ui.horizontal_wrapped(|ui| {
                for browser in browser_sites::Browser::ALL {
                    if ui
                        .add(
                            egui::Button::new(browser.name())
                                .selected(self.sites.browser == browser),
                        )
                        .clicked()
                        && self.sites.browser != browser
                    {
                        self.sites.browser = browser;
                        self.sites.discovery = Default::default();
                        self.sites.source.clear();
                    }
                }
            });
            if self.sites.browser != browser_sites::Browser::Firefox {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Порт CDP:");
                    ui.add(egui::DragValue::new(&mut self.sites.port).range(1..=65535));
                    ui.weak("127.0.0.1 — подключённый Chromium-браузер");
                });
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Найти сайты").clicked() {
                    self.sites.start(false);
                }
                if ui.button("Только история").clicked() {
                    self.sites.start(true);
                }
                if ui.button("Выбрать файл истории…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Выберите History (Chromium) или places.sqlite (Firefox)")
                        .pick_file()
                    {
                        let browser = self.sites.browser;
                        let (tx, rx) = mpsc::channel();
                        self.sites.rx = Some(rx);
                        self.sites.discovery = Default::default();
                        std::thread::spawn(move || {
                            let _ = tx.send(browser_sites::custom_history(path, browser));
                        });
                    }
                }
            });
        });
        if self.sites.rx.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Чтение сайтов…");
            });
        }
        ui.collapsing("Как подключить браузер", |ui| {
            ui.label("Chrome / Opera / Edge: CDP доступен при запуске браузера с --remote-debugging-port=9222. Chrome 136+ дополнительно требует --user-data-dir с отдельной папкой профиля. Программа не перезапускает браузер и не меняет ярлыки.");
            ui.label("CDP показывает вкладки браузера, подключённого к указанному порту; он может отличаться от выбранного выше браузера истории. Выберите нужный сайт в списке.");
            ui.label("Firefox: используется история. История показывает последние записанные посещения, которые могут отличаться от текущей вкладки. Приватные посещения в историю не попадают.");
        });
        for note in &self.sites.discovery.notes {
            ui.weak(note);
        }
            });
            settings_card(&mut columns[1], |ui| {
                section_title(ui, "Добавление домена");
        ui.add_space(6.);
        ui.label("Домен или адрес сайта");
        let input = ui.scope(|ui| {
            ui.visuals_mut().extreme_bg_color = egui::Color32::from_rgb(14, 18, 24);
            ui.add(egui::TextEdit::singleline(&mut self.sites.domain).desired_width(f32::INFINITY))
        }).inner;
        if input.changed() {
            self.sites.source = "Введён вручную".into();
            self.sites.message.clear();
        }
        if !self.sites.source.is_empty() {
            ui.weak(&self.sites.source);
        }
        let parsed = browser_sites::domain(&self.sites.domain);
        if let Ok(domain) = &parsed {
            ui.label(format!("Будет добавлен домен: {domain}"));
        }
        ui.horizontal_wrapped(|ui| {
            for (exclude, label) in [(false, "Через Zapret"), (true, "Исключить из Zapret")]
            {
                if ui
                    .add_enabled(parsed.is_ok(), egui::Button::new(label))
                    .clicked()
                    && self.save_editor()
                {
                    match list_store::add_domain(
                        self.config.root.as_ref().unwrap(),
                        &self.sites.domain,
                        exclude,
                    ) {
                        Ok(message) => {
                            self.sites.message = message.clone();
                            self.log = message;
                            if let Some(path) = self.selected.clone() {
                                self.open_list(path);
                            }
                        }
                        Err(error) => {
                            self.sites.message = error.clone();
                            self.log = error;
                        }
                    }
                }
            }
        });
        if !self.sites.message.is_empty() {
            ui.label(&self.sites.message);
        }
        ui.weak("Начальный www. убирается, остальные поддомены сохраняются. Повторы и прежняя запись с www. удаляются; домен убирается из противоположного user-листа.");
            });
        });
        ui.add_space(8.);
        section_title(ui, "Найденные сайты");
        let mut selected = None;
        for (index, site) in self.sites.discovery.sites.iter().enumerate() {
            let source = if site.visited > 0 {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let minutes = now.saturating_sub(site.visited as u64) / 60;
                let age = if minutes < 1 {
                    "меньше минуты назад".into()
                } else if minutes < 60 {
                    format!("{minutes} мин. назад")
                } else if minutes < 1440 {
                    format!("{} ч. назад", minutes / 60)
                } else {
                    format!("{} дн. назад", minutes / 1440)
                };
                format!("{} · {age}", site.source)
            } else {
                site.source.clone()
            };
            let title: String = site
                .title
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(100)
                .collect();
            let label = format!("{}\n{}\n{}", site.domain, title, source);
            if site_button(ui, label, self.sites.domain == site.domain)
                .on_hover_text(&site.url)
                .clicked()
            {
                selected = Some(index);
            }
        }
        if let Some(index) = selected {
            let site = &self.sites.discovery.sites[index];
            self.sites.domain = site.domain.clone();
            self.sites.source = site.source.clone();
            self.sites.message.clear();
        }
        if self.sites.discovery.sites.is_empty() && self.sites.rx.is_none() {
            ui.weak("Список пока пуст.");
        }
    }
    fn updates_ui(&mut self, ui: &mut egui::Ui) {
        page_title(ui, "Обновления Flowseal");
        settings_card(ui, |ui| {
            let current = updates::local(self.config.root.as_ref().unwrap());
            match &current {
                Ok(v) => {
                    ui.label(format!("Установленная версия: {v}"));
                }
                Err(e) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                }
            }
            if let Some(latest) = &self.updates.latest {
                ui.label(format!("Версия в источнике Flowseal: {latest}"));
                if let Ok(current) = &current {
                    match updates::compare(current, latest) {
                        Some(std::cmp::Ordering::Less) => {
                            ui.colored_label(
                                egui::Color32::from_rgb(235, 190, 100),
                                "Доступна новая версия",
                            );
                        }
                        Some(std::cmp::Ordering::Equal) => {
                            ui.colored_label(
                                egui::Color32::LIGHT_GREEN,
                                "Установленная версия совпадает с источником",
                            );
                        }
                        Some(std::cmp::Ordering::Greater) => {
                            ui.label("Установленная версия новее указанной в источнике");
                        }
                        _ => {}
                    }
                }
            } else {
                ui.weak("Доступная версия ещё не определена");
            }
            if ui
                .add_enabled(
                    self.updates.rx.is_none(),
                    egui::Button::new("Проверить обновления"),
                )
                .clicked()
            {
                self.updates.start();
            }
            if self.updates.rx.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Проверка источника Flowseal…");
                });
            }
            if !self.updates.error.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &self.updates.error);
            }
            if let Some(checked) = &self.updates.checked {
                ui.weak(checked);
            }
            ui.add_space(12.);
            if ui
                .add_enabled(
                    !self.tests.busy() && !self.tests.needs_recovery() && self.updates.rx.is_none(),
                    egui::Button::new("Обновить Zapret с сохранением списков"),
                )
                .clicked()
            {
                if self.save_editor() {
                    if !self.config_path.exists() {
                        if let Err(error) = fs::write(
                            &self.config_path,
                            serde_json::to_vec_pretty(&self.config).unwrap(),
                        ) {
                            self.updates.error = format!("Не удалось сохранить настройки: {error}");
                            return;
                        }
                    }
                    self.log = "Обновление Zapret: подтверждение Windows, загрузка релиза, перенос пользовательских списков и переключение папки…".into();
                    self.updates
                        .start_install(self.config.root.clone().unwrap(), self.config_path.clone());
                }
            }
            ui.hyperlink_to("Открыть страницу загрузки Flowseal", updates::DOWNLOAD);
            ui.weak("Обновление скачивает последний релиз GitHub в папку с номером версии и переносит все user-листы, их резервные копии и дополнительные списки. Старая папка сохраняется. Служба переключается на новую папку с сохранением её состояния. Отдельный батник перед обновлением закройте.");
            ui.weak("Совместимость функций приложения проверялась на Flowseal 1.10.4; наличие новой версии не означает проверенную совместимость с ней.");
        });
    }
    fn tests_ui(&mut self, ui: &mut egui::Ui) {
        page_title(ui, "Тестирование стратегий");
        ui.weak("HTTP/TLS/ping проверяет доступность сайтов. DPI TCP 16–20 проверяет возможное зависание передачи данных. Служба и отдельный winws из выбранной папки восстанавливаются после теста.");
        let busy = self.tests.busy();
        if self.tests.needs_recovery() && !busy {
            ui.colored_label(
                egui::Color32::YELLOW,
                "Обнаружен незавершённый тест. Сначала восстановите состояние.",
            );
            if ui.button("Восстановить исходное состояние").clicked() {
                self.tests.recover();
            }
        }
        if !self.tests.error.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_RED, &self.tests.error);
        }
        let root = self.config.root.clone().unwrap();
        ui.columns(2, |columns| {
            settings_card(&mut columns[0], |ui| {
            section_title(ui, "Стратегии");
            ui.add_enabled_ui(!busy && !self.tests.needs_recovery(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Все").clicked() {
                        self.tests.selected = backend::strategies(&root).into_iter().collect();
                    }
                    if ui.button("Снять выбор").clicked() {
                        self.tests.selected.clear();
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("test_strategies")
                    .max_height(230.)
                    .show(ui, |ui| {
                        for name in backend::strategies(&root) {
                            let mut selected = self.tests.selected.contains(&name);
                            if ui.checkbox(&mut selected, &name).changed() {
                                if selected {
                                    self.tests.selected.insert(name);
                                } else {
                                    self.tests.selected.remove(&name);
                                }
                            }
                        }
                    });
            });
            });
            settings_card(&mut columns[1], |ui| {
            section_title(ui, "Проверка");
            ui.add_enabled_ui(!busy,|ui|{ui.horizontal_wrapped(|ui|{for (value, label) in [(false, "HTTP / TLS / ping"), (true, "DPI TCP 16–20")] { let selected = self.tests.dpi == value; let fill = if selected { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.inactive.weak_bg_fill }; if ui.add(egui::Button::new(label).fill(fill)).clicked() { self.tests.dpi = value; } }});if self.tests.dpi {ui.label("Свой хост (пусто — набор dpi-checkers)");ui.text_edit_singleline(&mut self.tests.dpi_host);ui.horizontal(|ui|{ui.label("Размер данных, байт");ui.add(egui::DragValue::new(&mut self.tests.range_bytes).range(16384..=1048576));});ui.weak("IPSet временно переключается на любые IP и восстанавливается после теста. Возможна блокировка — это признак, а не доказательство.");}});
            ui.add_enabled_ui(!busy, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Таймаут HTTP, сек.");
                    ui.add(egui::DragValue::new(&mut self.tests.timeout).range(1..=20));
                });
            });
            if !self.tests.dpi { match testing::targets(&root) {
                Ok(t) => {
                    ui.label(format!("Целей: {}", t.len()));
                }
                Err(e) => {
                    ui.label(e);
                }
            }
            }
            if ui
                .add_enabled(
                    !busy && !self.tests.needs_recovery() && !self.tests.selected.is_empty(),
                    egui::Button::new("Запустить выбранные стратегии…"),
                )
                .clicked()
            {
                self.tests.confirm = true;
            }
            if ui
                .add_enabled(busy, egui::Button::new("Отменить и восстановить"))
                .clicked()
            {
                self.tests.cancel();
            }
            ui.weak("DPI: подробности объёма передачи и времени — при наведении на цель результата.");
            ui.label(format!(
                "Отчёт: {}",
                self.tests.dir.join("report.json").display()
            ));
            });
        });
        let report = &self.tests.report;
        ui.separator();
        ui.label(&report.message);
        if report.total > 0 {
            ui.add(
                egui::ProgressBar::new(report.completed as f32 / report.total as f32).text(
                    format!("{} / {} · {}", report.completed, report.total, report.phase),
                ),
            );
        }
        let mut apply = None;
        ui.horizontal_wrapped(|ui| {
            for (label, color) in [
                ("OK — успешно", egui::Color32::from_rgb(110, 210, 150)),
                (
                    "ERROR / SSL/DNS — ошибка",
                    egui::Color32::from_rgb(245, 120, 120),
                ),
                (
                    "UNSUP — не поддерживается; LIKELY_BLOCKED — возможная блокировка",
                    egui::Color32::from_rgb(235, 190, 100),
                ),
                ("— — не проверялось", egui::Color32::GRAY),
            ] {
                ui.colored_label(color, label);
            }
        });
        egui::ScrollArea::both()
            .id_salt("test_results")
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let mut groups: Vec<(&str, Vec<&testing::Row>)> = Vec::new();
                for row in &self.tests.report.rows {
                    if let Some((_, rows)) =
                        groups.iter_mut().find(|(name, _)| *name == row.strategy)
                    {
                        rows.push(row);
                    } else {
                        groups.push((&row.strategy, vec![row]));
                    }
                }
                for (name, rows) in groups {
                    let values = rows
                        .iter()
                        .flat_map(|r| [r.http.as_str(), r.tls12.as_str(), r.tls13.as_str()])
                        .collect::<Vec<_>>();
                    let ok = values.iter().filter(|v| **v == "OK").count();
                    let errors = values
                        .iter()
                        .filter(|v| matches!(**v, "ERROR" | "SSL/DNS" | "START ERROR"))
                        .count();
                    let unsupported = values.iter().filter(|v| **v == "UNSUP").count();
                let blocked=values.iter().filter(|v|**v=="LIKELY_BLOCKED").count();
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        egui::CollapsingHeader::new(egui::RichText::new(name).strong())
                            .id_salt(("strategy_results", name))
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.colored_label(result_color("OK"), format!("Успешно: {ok}"));
                                    ui.colored_label(
                                        result_color("ERROR"),
                                        format!("Ошибок: {errors}"),
                                    );
                                    ui.colored_label(
                                        result_color("UNSUP"),
                                        format!("Не поддерживается: {unsupported}; возможная блокировка: {blocked}"),
                                    );
                                    if ui
                                        .add_enabled(
                                            !busy
                                                && !self.tests.needs_recovery()
                                                && !rows.iter().any(|r| r.http == "START ERROR"),
                                            egui::Button::new("Применить стратегию…"),
                                        )
                                        .clicked()
                                    {
                                        apply = Some(name.to_owned());
                                    }
                                });
                                egui::Grid::new(("strategy_table", name))
                                    .striped(true)
                                    .spacing([18., 8.])
                                    .show(ui, |ui| {
                                        for label in ["Цель", "HTTP", "TLS 1.2", "TLS 1.3", "Ping"]
                                        {
                                            ui.strong(label);
                                        }
                                        ui.end_row();
                                        for row in &rows {
                                            ui.label(&row.target).on_hover_text(&row.detail);
                                            for value in
                                                [&row.http, &row.tls12, &row.tls13, &row.ping]
                                            {
                                                ui.colored_label(result_color(value), value);
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });
                    });
                    ui.add_space(8.);
                }
            });
        if let Some(name) = apply {
            self.strategy = name;
            self.confirm = Some("install".into());
        }
        if self.tests.confirm {
            egui::Window::new("Запуск тестирования").collapsible(false).show(ui.ctx(),|ui|{
                ui.label("В режиме DPI IPSet временно переключается на любые IP и затем восстанавливается. Тест временно остановит службу Zapret и отдельный winws из выбранной папки. Подключения могут прерваться. После завершения или отмены исходное состояние службы будет восстановлено. Права администратора запросит Windows.");
                ui.horizontal(|ui|{
                    if ui.button("Начать тест").clicked(){self.tests.confirm=false;if self.save_editor(){if let Err(e)=self.tests.start(&root){self.tests.error=e;}}}
                    if ui.button("Отмена").clicked(){self.tests.confirm=false;}
                });
            });
        }
    }
    fn fakes_ui(&mut self, ui: &mut egui::Ui) {
        page_title(ui, "Активные fake-файлы");
        settings_card(ui, |ui| {
            ui.weak("Выберите содержимое для Discord UDP или Game Filter UDP. Список соответствует .bin-файлам сборки. Перед заменой сохраняется предыдущий файл; применение требует перезапуска Zapret.");
            let root = self.config.root.as_ref().unwrap();
            match fakes::available(root) {
                Ok(files) => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.columns(2, |columns| {
                            for (column, (target, label)) in columns.iter_mut().zip(fakes::TARGETS)
                            {
                                section_title(column, label);
                                let active = fakes::current(root, target, &files);
                                column.label(format!(
                                    "Текущий: {}",
                                    if active.is_empty() {
                                        "Не совпадает с файлами сборки"
                                    } else {
                                        &active
                                    }
                                ));
                                if column
                                    .add_enabled(
                                        root.join("bin").join(format!("{target}.backup")).is_file(),
                                        egui::Button::new("Восстановить предыдущий"),
                                    )
                                    .clicked()
                                {
                                    self.fake_confirm = Some((target.into(), String::new()));
                                }
                                column.separator();
                                if files.is_empty() {
                                    column.weak("Доступные .bin-файлы не найдены");
                                }
                                for name in &files {
                                    if column
                                        .add_sized(
                                            [column.available_width(), 30.],
                                            egui::Button::new(name)
                                                .selected(active.split(", ").any(|n| n == name)),
                                        )
                                        .clicked()
                                    {
                                        self.fake_confirm = Some((target.into(), name.clone()));
                                    }
                                }
                            }
                        });
                    });
                }
                Err(e) => {
                    ui.label(format!("Не удалось прочитать fake-файлы: {e}"));
                }
            }
        });
    }
    fn ipset_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.);
        ui.separator();
        settings_card(ui, |ui| {
            section_title(ui, "IPSet Filter");
            let root = self.config.root.as_ref().unwrap();
            let state = ipset::status(root);
            let current = state.as_ref().copied().unwrap_or("unknown");
            ui.label(format!(
                "Текущий режим: {}",
                match current {
                    "loaded" => "По списку",
                    "none" => "Отключён",
                    "any" => "Любые IP",
                    _ => "Не удалось прочитать список",
                }
            ));
            if let Err(e) = state {
                ui.weak(e);
            }
            ui.weak("По списку — IPSet-правила используют адреса из файла. Отключён — файл содержит служебный адрес. Любые IP — файл пустой, IPSet-правила могут охватывать любые адреса.");
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [
                    ("loaded", "По списку"),
                    ("none", "Отключён"),
                    ("any", "Любые IP"),
                ] {
                    if ui
                        .add_enabled(
                            current != mode,
                            egui::Button::new(label).selected(current == mode),
                        )
                        .clicked()
                    {
                        self.launch(format!("ipset:{mode}"));
                    }
                }
            });
            if ui.button("Обновить список Flowseal").clicked() {
                self.launch("ipset:update".into());
            }
            ui.weak("Резервная копия: lists/ipset-all.txt.backup. Обновление сохраняет выбранный режим. Настройка относится к IPSet-правилам стратегии; доменные правила работают отдельно.");
        });
    }
    fn launch(&mut self, action: String) {
        if self.tests.busy() || self.tests.needs_recovery() {
            self.log =
                "Сначала завершите тест или восстановите состояние на вкладке «Тесты».".into();
            return;
        }
        if let Some(root) = self.config.root.clone() {
            if action == "install" {
                if let Err(e) = backend::strategy_args(&root, &self.strategy) {
                    self.log = e;
                    return;
                }
            }
            if action.starts_with("ipset:") && !self.save_editor() {
                return;
            }
            self.diagnostic_pending = action == "diagnostics";
            self.repair_pending = action.starts_with("fix_");
            let strategy = self.strategy.clone();
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            self.log = "Ожидание выполнения и подтверждения Windows…".into();
            std::thread::spawn(move || {
                let result = if action == "status" {
                    backend::status()
                } else if let Some(rest) = action.strip_prefix("fake:") {
                    if let Some((target, source)) = rest.split_once(':') {
                        let result = if source.is_empty() {
                            fakes::restore(&root, target)
                        } else {
                            fakes::replace(&root, target, source)
                        };
                        result.unwrap_or_else(|e| format!("Ошибка fake-файла: {e}"))
                    } else {
                        "Неверная операция fake-файла".into()
                    }
                } else if let Some(mode) = action.strip_prefix("ipset:") {
                    let result = if mode == "update" {
                        ipset::update(&root)
                    } else {
                        ipset::set_mode(&root, mode)
                    };
                    result.unwrap_or_else(|e| format!("Ошибка IPSet: {e}"))
                } else if action.starts_with("fix_") {
                    let result = backend::elevated(action, root.clone(), strategy)
                        .unwrap_or_else(|e| format!("Ошибка исправления: {e}"));
                    serde_json::to_string(&(result, backend::diagnostics(&root)))
                        .unwrap_or_default()
                } else if action == "diagnostics" {
                    serde_json::to_string(&backend::diagnostics(&root)).unwrap_or_default()
                } else {
                    backend::elevated(action, root, strategy)
                        .unwrap_or_else(|e| format!("Ошибка: {e}"))
                };
                let _ = tx.send(result);
            });
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.tests.poll();
        self.sites.poll();
        if self.sites.rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.updates.poll();
        if let Some(rx) = &self.updates.install_rx {
            let result = match rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("Обновление прервано. Повторное обновление восстановит предыдущую настройку при необходимости.".into())),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.updates.install_rx = None;
                match result {
                    Ok(path) => {
                        self.config.root = Some(PathBuf::from(&path));
                        self.selected = None;
                        self.last_edit = None;
                        self.load();
                        self.snapshot_time = None;
                        self.log = format!("Zapret обновлён: {path}. Пользовательские списки сохранены. Предыдущая папка оставлена для резервной копии.");
                    }
                    Err(error) => {
                        self.updates.error = error;
                        self.log =
                            "Обновление не завершено. Подробности в настройках → Обновления."
                                .into();
                    }
                }
            }
        }
        if self.updates.install_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        if self.updates.rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if self.tests.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.tests.busy() {
            self.tests.cancel();
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.log="Отмена теста и восстановление состояния. Дождитесь завершения перед закрытием окна.".into();
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.save_editor() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if let Some(time) = self.last_edit {
            let delay = std::time::Duration::from_millis(400);
            if time.elapsed() >= delay {
                self.save_editor();
            } else {
                ctx.request_repaint_after(delay - time.elapsed());
            }
        }
        if let Some(rx) = &self.rx {
            if let Ok(s) = rx.try_recv() {
                if self.repair_pending {
                    match serde_json::from_str::<(String, Vec<backend::Check>)>(&s) {
                        Ok((result, checks)) => {
                            self.log = result;
                            self.report = checks;
                        }
                        Err(e) => self.log = format!("Ошибка результата исправления: {e}"),
                    }
                    self.repair_pending = false;
                } else if self.diagnostic_pending {
                    match serde_json::from_str(&s) {
                        Ok(report) => {
                            self.report = report;
                            self.log =
                                "Диагностика завершена. Отчёт доступен на вкладке «Диагностика»."
                                    .into();
                        }
                        Err(e) => self.log = format!("Ошибка отчёта диагностики: {e}"),
                    }
                    self.diagnostic_pending = false;
                } else {
                    self.log = s;
                    if self
                        .selected
                        .as_ref()
                        .is_some_and(|p| p.file_name().is_some_and(|n| n == "ipset-all.txt"))
                        && self.last_edit.is_none()
                    {
                        if let Some(p) = self.selected.clone() {
                            self.open_list(p);
                        }
                    }
                }
                self.rx = None;
                self.snapshot_time = None;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(rx) = &self.snapshot_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(snapshot) => {
                        self.snapshot = Some(snapshot);
                        self.snapshot_error.clear()
                    }
                    Err(e) => self.snapshot_error = e,
                };
                self.snapshot_rx = None;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if self
            .snapshot_time
            .is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(10))
        {
            self.refresh_snapshot();
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(10));
        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(36, 42, 51))
                    .inner_margin(egui::Margin::symmetric(20., 12.)),
            )
            .show(ctx, |ui| {
                self.status_ui(ui);
            });
        egui::TopBottomPanel::bottom("log").show(ctx, |ui| {
            let error = self.log.to_lowercase().contains("ошиб") || self.log.contains("Exception");
            let title = if error {
                "Журнал событий · требуется внимание"
            } else {
                "Журнал событий"
            };
            egui::CollapsingHeader::new(title)
                .id_salt("journal_disclosure")
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(160.)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.log)
                                    .interactive(false)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                });
            if error {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    self.log.lines().next().unwrap_or("Ошибка"),
                );
            }
        });
        egui::SidePanel::left("nav")
            .exact_width(170.)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(31, 36, 44))
                    .inner_margin(12.),
            )
            .show(ctx, |ui| {
                ui.add_space(10.);
                for (id, name) in [
                    (0, "Управление"),
                    (7, "Добавить сайт"),
                    (2, "Списки"),
                    (3, "Тесты"),
                    (4, "Диагностика"),
                    (1, "Настройки"),
                ] {
                    let selected = if id == 1 {
                        [1, 5, 6].contains(&self.tab)
                    } else {
                        self.tab == id
                    };
                    if ui
                        .add_sized(
                            [ui.available_width(), 40.],
                            egui::Button::new(name).selected(selected),
                        )
                        .clicked()
                    {
                        self.tab = id;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.weak("GUI-for-zapret-FS · 0.1");
                    ui.label("Flowseal 1.10.4");
                    ui.weak("Совместимость");
                });
            });
        if self.tab == 0 || self.tab == 2 {
            egui::SidePanel::left("files")
                .default_width(238.)
                .width_range(200. ..=440.)
                .show(ctx, |ui| {
                    section_title(
                        ui,
                        if self.tab == 0 {
                            "Стратегии"
                        } else {
                            "Листы"
                        },
                    );
                    ui.separator();
                    ui.add_enabled_ui(
                        self.rx.is_none() && self.updates.install_rx.is_none(),
                        |ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                if self.tab == 0 {
                                    if let Some(root) = &self.config.root {
                                        for name in backend::strategies(root) {
                                            if ui
                                                .add_sized(
                                                    [ui.available_width(), 30.],
                                                    egui::Button::new(
                                                        name.trim_end_matches(".bat"),
                                                    )
                                                    .selected(self.strategy == name),
                                                )
                                                .clicked()
                                            {
                                                self.strategy = name;
                                            }
                                        }
                                    }
                                } else {
                                    for user in [true, false] {
                                        ui.add_space(8.);
                                        ui.label(
                                            egui::RichText::new(if user {
                                                "USER-ЛИСТЫ"
                                            } else {
                                                "ПРОЧИЕ ЛИСТЫ"
                                            })
                                            .small()
                                            .strong()
                                            .weak(),
                                        );
                                        for path in self.files.clone() {
                                            let name = path
                                                .file_name()
                                                .unwrap()
                                                .to_string_lossy()
                                                .into_owned();
                                            if name.to_lowercase().contains("user") != user {
                                                continue;
                                            }
                                            if ui
                                                .add_sized(
                                                    [ui.available_width(), 30.],
                                                    egui::Button::new(&name).selected(
                                                        self.selected.as_ref() == Some(&path),
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                self.open_list(path);
                                            }
                                        }
                                        ui.separator();
                                    }
                                }
                            });
                        },
                    );
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(29, 33, 40))
                    .inner_margin(22.),
            )
            .show(ctx, |ui| {
                if self.config.root.is_none() {
                    page_title(ui, "Выберите папку Zapret");
                    if ui.button("Выбрать папку…").clicked() {
                        self.pick_folder();
                    }
                    return;
                }
                if [1, 5, 6].contains(&self.tab) {
                    ui.horizontal_wrapped(|ui| {
                        for (id, name) in [(1, "Фильтры"), (5, "Fake-файлы"), (6, "Обновления")]
                        {
                            let fill = if self.tab == id {
                                ui.visuals().selection.bg_fill
                            } else {
                                ui.visuals().widgets.inactive.weak_bg_fill
                            };
                            if ui.add(egui::Button::new(name).fill(fill)).clicked() {
                                self.tab = id;
                            }
                        }
                    });
                    ui.separator();
                }
                ui.add_enabled_ui(
                    self.rx.is_none()
                        && self.updates.install_rx.is_none()
                        && self.config.root.is_some()
                        && (self.tab == 3 || (!self.tests.busy() && !self.tests.needs_recovery())),
                    |ui| {
                        if [0, 1, 6, 7].contains(&self.tab) {
                            egui::ScrollArea::vertical()
                                .id_salt(("workspace", self.tab))
                                .show(ui, |ui| self.workspace(ui, ctx));
                        } else {
                            self.workspace(ui, ctx);
                        }
                    },
                );
            });
        if let Some((target, source)) = self.fake_confirm.clone() {
            egui::Window::new("Замена fake-файла").collapsible(false).resizable(false).show(ctx,|ui|{
                ui.label(if source.is_empty(){format!("Восстановить предыдущий {target} из резервной копии?")}else{format!("Заменить {target} содержимым {source}? Предыдущий файл будет сохранён в .backup.")});
                ui.label("Для применения потребуется перезапуск Zapret.");
                ui.horizontal(|ui|{
                    if ui.button("Выполнить").clicked(){self.fake_confirm=None;self.launch(format!("fake:{target}:{source}"));}
                    if ui.button("Отмена").clicked(){self.fake_confirm=None;}
                });
            });
        }
        if let Some(action) = self.confirm.clone() {
            egui::Window::new("Подтверждение действия").collapsible(false).resizable(false).show(ctx,|ui|{ui.label(match action.as_str(){"install"=>"Применить выбранную стратегию? Текущая служба будет остановлена и перенастроена.","remove"=>"Остановить и удалить службу zapret? Файлы сохранятся.","fix_bfe"=>"Запустить службу базовой фильтрации Windows (BFE)? Она нужна для работы WinDivert. Тип запуска службы не меняется.","fix_timestamps"=>"Включить глобальный параметр TCP timestamps в Windows, как это делает service.bat? Это системная настройка для всех приложений.",_=>"Запустить оригинальный тест с правами администратора? Он управляет процессами winws и IPSet."});ui.horizontal(|ui|{if ui.button("Выполнить").clicked(){self.confirm=None;if action=="tests"{let p=self.config.root.as_ref().unwrap().join("utils/test zapret.ps1");let script=format!("Start-Process powershell.exe -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File \"{}\"'",p.display().to_string().replace('\'',"''"));self.log=match backend::command("powershell.exe").args(["-NoProfile","-Command",&script]).spawn(){Ok(_)=>"Запрошен запуск тестов в отдельном окне".into(),Err(e)=>e.to_string()};}else{self.launch(action.clone())}}if ui.button("Отмена").clicked(){self.confirm=None;}});});
        }
    }
}
fn settings_card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(12.)
        .rounding(6.)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            content(ui);
        });
}
fn site_button(ui: &mut egui::Ui, label: String, selected: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 76.), egui::Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    ui.painter().rect(
        rect,
        visuals.rounding,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
    );
    let text_rect = rect.shrink2(egui::vec2(12., 8.));
    let galley = ui.painter().layout(
        label,
        egui::FontId::proportional(13.),
        visuals.text_color(),
        text_rect.width(),
    );
    ui.painter()
        .with_clip_rect(text_rect)
        .galley(text_rect.min, galley, visuals.text_color());
    response
}
fn page_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(22.)
            .strong()
            .color(egui::Color32::from_rgb(119, 184, 250)),
    );
}
fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).size(17.).strong());
}
fn result_color(value: &str) -> egui::Color32 {
    match value {
        "OK" => egui::Color32::from_rgb(110, 210, 150),
        "ERROR" | "SSL/DNS" | "START ERROR" => egui::Color32::from_rgb(245, 120, 120),
        "UNSUP" | "Timeout" | "LIKELY_BLOCKED" => egui::Color32::from_rgb(235, 190, 100),
        "—" => egui::Color32::GRAY,
        _ if value.ends_with(" ms") => egui::Color32::from_rgb(130, 195, 235),
        _ => egui::Color32::LIGHT_GRAY,
    }
}
fn main() -> eframe::Result<()> {
    let a: Vec<_> = std::env::args().collect();
    if a.get(1).is_some_and(|s| s == "--helper") {
        if a.len() == 6 {
            let result = backend::helper(&a[2], &PathBuf::from(&a[3]), &a[4]);
            if let Ok(bytes) = serde_json::to_vec(&result) {
                let _ = fs::write(&a[5], bytes);
            }
        }
        return Ok(());
    }
    eframe::run_native(
        "GUI-for-zapret-FS",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_icon(
                    eframe::icon_data::from_png_bytes(include_bytes!("../logo1.png"))
                        .expect("Invalid application logo"),
                )
                .with_inner_size([1280., 720.])
                .with_min_inner_size([1000., 620.]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
