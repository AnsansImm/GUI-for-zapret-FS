#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod backend;
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
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
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
        };
        a.load();
        a
    }
    fn load(&mut self) {
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
    fn launch(&mut self, action: String) {
        if let Some(root) = self.config.root.clone() {
            if action == "install" {
                if let Err(e) = backend::strategy_args(&root, &self.strategy) {
                    self.log = e;
                    return;
                }
            }
            let strategy = self.strategy.clone();
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            self.log = "Ожидание выполнения и подтверждения Windows…".into();
            std::thread::spawn(move || {
                let result = if action == "status" {
                    backend::status()
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
        if let Some(rx) = &self.rx {
            if let Ok(s) = rx.try_recv() {
                self.log = s;
                self.rx = None;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Zapret GUI");
                if ui
                    .add_enabled(self.rx.is_none(), egui::Button::new("Выбрать папку"))
                    .clicked()
                {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        if p.join("service.bat").is_file() && p.join("bin/winws.exe").is_file() {
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
                for (i, n) in ["Управление", "Настройки", "Списки", "Тесты"]
                    .iter()
                    .enumerate()
                {
                    ui.selectable_value(&mut self.tab, i, *n);
                }
                ui.separator();
                ui.label("Совместимость: Flowseal 1.10.4");
            });
        egui::CentralPanel::default().show(ctx,|ui|{ui.add_enabled_ui(self.rx.is_none()&&self.config.root.is_some(),|ui|{match self.tab{
 0=>{ui.heading("Служба Zapret");ui.label("Стратегия для установки:");if let Some(root)=&self.config.root{egui::ComboBox::from_id_salt("strategy").selected_text(&self.strategy).show_ui(ui,|ui|{for n in backend::strategies(root){ui.selectable_value(&mut self.strategy,n.clone(),n);}});}ui.horizontal(|ui|{for(action,label)in[("status","Обновить состояние"),("start","Запустить"),("stop","Остановить")]{if ui.button(label).clicked(){self.launch(action.into())}}});if ui.button("Установить / применить стратегию").clicked(){self.confirm=Some("install".into())}if ui.button("Удалить службу Zapret").clicked(){self.confirm=Some("remove".into())}ui.label("Установка задаёт автозапуск службы и включает TCP timestamps. Удаление касается только службы zapret.");},
 1=>{ui.heading("Game Filter");egui::ComboBox::from_id_salt("mode").selected_text(&self.mode).show_ui(ui,|ui|{for m in["disabled","all","tcp","udp"]{ui.selectable_value(&mut self.mode,m.into(),m);}});ui.label("TCP: порты и диапазоны через запятую");ui.text_edit_singleline(&mut self.tcp);ui.label("UDP");ui.text_edit_singleline(&mut self.udp);if ui.button("Сохранить настройки").clicked(){if backend::valid_ports(&self.tcp)&&backend::valid_ports(&self.udp){let p=self.config.root.as_ref().unwrap().join("utils/game_filter.enabled");self.log=match fs::write(p,format!("mode={}\ntcp={}\nudp={}\n",self.mode,self.tcp,self.udp)){Ok(_)=>"Сохранено. Для службы повторно примените выбранную стратегию на вкладке «Управление».".into(),Err(e)=>e.to_string()};}else{self.log="Проверьте порты: допустимы значения 1–65535 и возрастающие диапазоны.".into()}}},
 2=>{ui.heading("Редактор списков");egui::ComboBox::from_id_salt("lists").selected_text(self.selected.as_ref().map(|p|p.file_name().unwrap().to_string_lossy().into_owned()).unwrap_or_else(||"Выберите список".into())).show_ui(ui,|ui|{for p in self.files.clone(){if ui.selectable_label(self.selected.as_ref()==Some(&p),p.file_name().unwrap().to_string_lossy()).clicked(){match fs::read_to_string(&p){Ok(t)=>{self.text=t.clone();self.disk=t;self.selected=Some(p)},Err(e)=>self.log=e.to_string()}}}});ui.label("Автосохранение. Комментарии начинаются с #. Исходный текст сохраняется без фильтрации.");if self.selected.is_some(){let response=egui::ScrollArea::vertical().show(ui,|ui|ui.add(egui::TextEdit::multiline(&mut self.text).desired_width(f32::INFINITY).desired_rows(22).font(egui::TextStyle::Monospace))).inner;if response.changed(){let p=self.selected.as_ref().unwrap();match fs::read_to_string(p){Ok(current)if current==self.disk=>{match fs::write(p,&self.text){Ok(_)=>{self.disk=self.text.clone();self.log="Сохранено".into()},Err(e)=>self.log=e.to_string()}},_=>{self.log="Файл изменён другой программой. Повторно выберите его перед редактированием. Ваш текст остаётся в редакторе.".into();}}}}},
 _=>{ui.heading("Тестирование стратегий");ui.label("В этой версии тесты запускаются в оригинальном окне PowerShell. Скрипт может останавливать winws и временно менять IPSet. Перед запуском остановите службу Zapret.");if ui.button("Открыть оригинальные тесты…").clicked(){self.confirm=Some("tests".into())}ui.label("Отчёты: utils / test results в папке Zapret. Полностью графическое тестирование — следующий этап.");}
 }});});
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
