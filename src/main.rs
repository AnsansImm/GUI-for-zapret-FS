#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod backend;
mod editor;
mod fakes;
mod ipset;
mod testing;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::mpsc};
#[derive(Default, Serialize, Deserialize)]
struct Config {
    root: Option<PathBuf>,
}
struct App {
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
    fake_confirm: Option<(String, String)>,
    tests: testing::Tests,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10., 10.);
        style.spacing.button_padding = egui::vec2(14., 9.);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(38, 91, 135);
        style.visuals.panel_fill = egui::Color32::from_rgb(24, 28, 35);
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
            fake_confirm: None,
            tests: testing::Tests::new(),
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
        match fs::read_to_string(path) {
            Ok(current) if current == self.disk => match fs::write(path, &content) {
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
            },
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
        ui.heading("Редактор списков");
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
                range.secondary.index = map(range.secondary.index).min(self.text.chars().count());
                let mut state = output.state;
                state.cursor.set_char_range(Some(range));
                state.store(ctx, id);
            }
            self.last_edit = Some(std::time::Instant::now());
            self.log = "Изменения будут сохранены…".into();
        }
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
    fn tests_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Тестирование стратегий");
        ui.label("HTTP / TLS 1.2 / TLS 1.3 / ping — цели из utils/targets.txt. IPSet не меняется. Служба будет временно остановлена и восстановлена после теста. Отдельный winws из выбранной папки будет сохранён, остановлен и восстановлен.");
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
            columns[0].heading("Стратегии");
            columns[0].add_enabled_ui(!busy && !self.tests.needs_recovery(), |ui| {
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
            let ui = &mut columns[1];
            ui.heading("Проверка");
            ui.add_enabled_ui(!busy, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Таймаут HTTP, сек.");
                    ui.add(egui::DragValue::new(&mut self.tests.timeout).range(1..=20));
                });
            });
            match testing::targets(&root) {
                Ok(t) => {
                    ui.label(format!("Целей: {}", t.len()));
                }
                Err(e) => {
                    ui.label(e);
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
            ui.weak("DPI-проверки пока доступны через оригинальный скрипт.");
            ui.label(format!(
                "Отчёт: {}",
                self.tests.dir.join("report.json").display()
            ));
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
        egui::ScrollArea::both()
            .id_salt("test_results")
            .show(ui, |ui| {
                egui::Grid::new("test_table").striped(true).show(ui, |ui| {
                    for label in [
                        "Стратегия",
                        "Цель",
                        "HTTP",
                        "TLS 1.2",
                        "TLS 1.3",
                        "Ping",
                        "",
                    ] {
                        ui.strong(label);
                    }
                    ui.end_row();
                    for row in &self.tests.report.rows {
                        ui.label(&row.strategy);
                        ui.label(&row.target);
                        for value in [&row.http, &row.tls12, &row.tls13, &row.ping] {
                            ui.label(value);
                        }
                        if ui
                            .add_enabled(
                                !busy && !self.tests.needs_recovery(),
                                egui::Button::new("Применить…"),
                            )
                            .clicked()
                        {
                            apply = Some(row.strategy.clone());
                        }
                        ui.end_row();
                    }
                });
            });
        if let Some(name) = apply {
            self.strategy = name;
            self.confirm = Some("install".into());
        }
        if self.tests.confirm {
            egui::Window::new("Запуск тестирования").collapsible(false).show(ui.ctx(),|ui|{
                ui.label("Тест временно остановит службу Zapret и отдельный winws из выбранной папки. Подключения могут прерваться. После завершения или отмены исходное состояние службы будет восстановлено. Права администратора запросит Windows.");
                ui.horizontal(|ui|{
                    if ui.button("Начать тест").clicked(){self.tests.confirm=false;if self.save_editor(){if let Err(e)=self.tests.start(&root){self.tests.error=e;}}}
                    if ui.button("Отмена").clicked(){self.tests.confirm=false;}
                });
            });
        }
    }
    fn fakes_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Активные fake-файлы");
        ui.label("Выберите содержимое для Discord UDP или Game Filter UDP. Список соответствует .bin-файлам сборки. Перед заменой сохраняется предыдущий файл; применение требует перезапуска Zapret.");
        let root = self.config.root.as_ref().unwrap();
        match fakes::available(root) {
            Ok(files) => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.columns(2, |columns| {
                        for (column, (target, label)) in columns.iter_mut().zip(fakes::TARGETS) {
                            column.heading(label);
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
    }
    fn ipset_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.);
        ui.separator();
        ui.heading("IPSet Filter");
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
        ui.label("По списку — IPSet-правила используют адреса из файла. Отключён — файл содержит служебный адрес. Любые IP — файл пустой, IPSet-правила могут охватывать любые адреса.");
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
                if self.diagnostic_pending {
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
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Zapret GUI");
                if ui
                    .add_enabled(
                        self.rx.is_none() && !self.tests.busy() && !self.tests.needs_recovery(),
                        egui::Button::new("Выбрать папку"),
                    )
                    .clicked()
                {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        if p.join("service.bat").is_file() && p.join("bin/winws.exe").is_file() {
                            if !self.save_editor() {
                                return;
                            }
                            self.config.root = Some(p);
                            self.selected = None;
                            self.load();
                            match fs::write(
                                &self.config_path,
                                serde_json::to_vec_pretty(&self.config).unwrap(),
                            ) {
                                Ok(_) => self.log = "Папка сохранена".into(),
                                Err(e) => self.log = e.to_string(),
                            }
                        } else {
                            self.log = "В папке не найдены service.bat и bin/winws.exe".into()
                        }
                    }
                }
            });
            ui.label(
                self.config
                    .root
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Папка не выбрана".into()),
            );
        });
        egui::TopBottomPanel::bottom("log")
            .resizable(true)
            .default_height(160.)
            .show(ctx, |ui| {
                ui.label("Журнал");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.log)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY),
                    );
                });
            });
        egui::SidePanel::left("nav")
            .default_width(170.)
            .show(ctx, |ui| {
                for (i, n) in [
                    "Управление",
                    "Настройки",
                    "Списки",
                    "Тесты",
                    "Диагностика",
                    "Fake-файлы",
                ]
                .iter()
                .enumerate()
                {
                    ui.selectable_value(&mut self.tab, i, *n);
                }
                ui.separator();
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_min_width(140.);
                        ui.label(egui::RichText::new("Совместимость").small().weak());
                        ui.label("Flowseal 1.10.4");
                    });
                });
            });
        if self.tab == 0 || self.tab == 2 {
            egui::SidePanel::left("files")
                .default_width(275.)
                .width_range(200. ..=440.)
                .show(ctx, |ui| {
                    ui.heading(if self.tab == 0 {
                        "Стратегии"
                    } else {
                        "Листы"
                    });
                    ui.separator();
                    ui.add_enabled_ui(self.rx.is_none(), |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            if self.tab == 0 {
                                if let Some(root) = &self.config.root {
                                    for name in backend::strategies(root) {
                                        if ui
                                            .add_sized(
                                                [ui.available_width(), 30.],
                                                egui::Button::new(name.trim_end_matches(".bat"))
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
                    });
                });
        }
        egui::CentralPanel::default().show(ctx,|ui|{ui.add_enabled_ui(self.rx.is_none()&&self.config.root.is_some()&&(self.tab==3||(!self.tests.busy()&&!self.tests.needs_recovery())),|ui|{match self.tab{
 0=>{ui.heading("Служба Zapret");ui.label(format!("Выбрана стратегия: {}",self.strategy.trim_end_matches(".bat")));ui.add_space(12.);ui.horizontal(|ui|{for(action,label)in[("status","Обновить состояние"),("start","Запустить"),("stop","Остановить")]{if ui.button(label).clicked(){self.launch(action.into())}}});if ui.button("Установить / применить стратегию").clicked(){self.confirm=Some("install".into())}if ui.button("Удалить службу Zapret").clicked(){self.confirm=Some("remove".into())}ui.label("Установка задаёт автозапуск службы и включает TCP timestamps. Удаление касается только службы zapret.");},
 1=>{ui.heading("Game Filter");ui.horizontal_wrapped(|ui|{for(value,label)in[("disabled","Отключён"),("all","TCP и UDP"),("tcp","TCP"),("udp","UDP")]{ui.selectable_value(&mut self.mode,value.into(),label);}});ui.label("TCP: порты и диапазоны через запятую");ui.text_edit_singleline(&mut self.tcp);ui.label("UDP");ui.text_edit_singleline(&mut self.udp);if ui.button("Сохранить настройки").clicked(){if backend::valid_ports(&self.tcp)&&backend::valid_ports(&self.udp){let p=self.config.root.as_ref().unwrap().join("utils/game_filter.enabled");self.log=match fs::write(p,format!("mode={}\ntcp={}\nudp={}\n",self.mode,self.tcp,self.udp)){Ok(_)=>"Сохранено. Для службы повторно примените выбранную стратегию на вкладке «Управление».".into(),Err(e)=>e.to_string()};}else{self.log="Проверьте порты: допустимы значения 1–65535 и возрастающие диапазоны.".into()}}self.ipset_ui(ui);},
 2=>{self.editor_ui(ui,ctx);},
 3=>{self.tests_ui(ui);},
 4=>{
 ui.heading("Диагностика");
 ui.label("Проверка не меняет настройки Windows и файлы Zapret. Предупреждения указывают на возможную причину, а не подтверждают неисправность.");
 ui.horizontal(|ui| {
 if ui.button("Запустить проверку").clicked() { self.launch("diagnostics".into()); }
 if ui.add_enabled(!self.report.is_empty(),egui::Button::new("Копировать отчёт")).clicked() {
 ui.ctx().copy_text(self.report.iter().map(|c|format!("[{}] {}: {}",c.level,c.title,c.detail)).collect::<Vec<_>>().join("\n"));
 }
 });
 ui.separator();
 if self.report.is_empty() { ui.weak("Проверка ещё не запускалась."); }
 egui::ScrollArea::vertical().show(ui,|ui| {
 for check in &self.report {
 let (label,color)=match check.level.as_str(){"ok"=>("В порядке",egui::Color32::from_rgb(115,200,150)),"warn"=>("Внимание",egui::Color32::from_rgb(235,190,100)),"unknown"=>("Не проверено",egui::Color32::from_rgb(230,140,140)),_=>("Информация",egui::Color32::from_rgb(140,185,230))};
 egui::Frame::group(ui.style()).show(ui,|ui|{ui.set_min_width(ui.available_width());ui.horizontal_wrapped(|ui|{ui.strong(&check.title);ui.colored_label(color,label);});ui.label(&check.detail);});
 }
 });
 }
 _=>{self.fakes_ui(ui);}
 }});});
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
            egui::Window::new("Подтверждение действия").collapsible(false).resizable(false).show(ctx,|ui|{ui.label(match action.as_str(){"install"=>"Применить выбранную стратегию? Текущая служба будет остановлена и перенастроена.","remove"=>"Остановить и удалить службу zapret? Файлы сохранятся.",_=>"Запустить оригинальный тест с правами администратора? Он управляет процессами winws и IPSet."});ui.horizontal(|ui|{if ui.button("Выполнить").clicked(){self.confirm=None;if action=="tests"{let p=self.config.root.as_ref().unwrap().join("utils/test zapret.ps1");let script=format!("Start-Process powershell.exe -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File \"{}\"'",p.display().to_string().replace('\'',"''"));self.log=match backend::command("powershell.exe").args(["-NoProfile","-Command",&script]).spawn(){Ok(_)=>"Запрошен запуск тестов в отдельном окне".into(),Err(e)=>e.to_string()};}else{self.launch(action.clone())}}if ui.button("Отмена").clicked(){self.confirm=None;}});});
        }
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
        "Zapret GUI",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1000., 720.]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
