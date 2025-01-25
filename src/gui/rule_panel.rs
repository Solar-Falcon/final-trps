use super::{ContentType, RuleData, RuleType};
use eframe::egui;

#[derive(Debug, Default)]
pub struct UiRulePanel {
    cursor: usize,
    rules: Vec<RuleData>,
}

impl UiRulePanel {
    pub fn display(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                self.display_rule_creation(ui);
            });

            ui.separator();

            self.display_rule_list(ui);
        });

        ui.separator();

        self.display_main_panel(ui);
    }

    #[inline]
    pub fn rules(&self) -> &Vec<RuleData> {
        &self.rules
    }

    fn display_rule_creation(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_label("Список правил")
            .width(250.0)
            .show_index(ui, &mut self.cursor, self.rules.len(), |i| {
                self.rules
                    .get(i)
                    .map(|rule| rule.name.as_str())
                    .unwrap_or("Ничего нет!")
            });

        if ui.button("Добавить в конец списка").clicked() {
            self.rules.push(Default::default());
            self.cursor = self.rules.len() - 1; // cursor on the new rule
        }

        if !self.rules.is_empty() {
            if ui.button("Добавить после выбранного").clicked() {
                self.cursor += 1;
                self.rules.insert(self.cursor, Default::default());
            }

            if ui.button("Добавить перед выбранным").clicked() {
                self.rules.insert(self.cursor, Default::default());
            }
        }

        if ui.button("Удалить выбранное правило").clicked() {
            self.rules.remove(self.cursor);
            self.shift_cursor_up(); // when we remove the last rule, cursor points to nothing
        }
    }

    fn display_main_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            if self.cursor < self.rules.len() {
                let rule = &mut self.rules[self.cursor];

                ui.horizontal(|ui| {
                    ui.label("Название: ");
                    ui.text_edit_singleline(&mut rule.name);
                });

                ui.horizontal(|ui| {
                    ui.label("Тип параметра: ");
                    ui.radio_value(&mut rule.rule_type, RuleType::Input, "Входной");
                    ui.radio_value(&mut rule.rule_type, RuleType::Output, "Выходной");
                });

                ui.horizontal(|ui| {
                    ui.label("Тип данных: ");
                    ui.radio_value(&mut rule.content_type, ContentType::PlainText, "Текст");
                    ui.radio_value(
                        &mut rule.content_type,
                        ContentType::Regex,
                        "Регулярное выражение",
                    );
                    ui.radio_value(
                        &mut rule.content_type,
                        ContentType::IntRanges,
                        "Целые числа",
                    );
                });

                let text_edit = egui::TextEdit::singleline(&mut rule.text)
                    .code_editor()
                    .desired_width(480.0);

                ui.add(text_edit);
            }
        });
    }

    fn display_rule_list(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.label("Навигация");

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if ui.button("Вверх").clicked() {
                        self.shift_cursor_up();
                    }
                    if ui.button("Вниз").clicked() {
                        self.shift_cursor_down();
                    }
                });

                ui.vertical(|ui| {
                    let current = self.cursor;

                    if ui.button("Сдвинуть вверх").clicked() {
                        self.shift_cursor_up();
                        self.rules.swap(current, self.cursor);
                    }

                    if ui.button("Сдвинуть вниз").clicked() {
                        self.shift_cursor_down();
                        self.rules.swap(current, self.cursor);
                    }
                });
            });

            ui.separator();

            for (i, rule) in self.rules.iter().enumerate() {
                if i == self.cursor {
                    ui.label(format!("> {} ({})", &rule.name, &rule.rule_type));
                } else {
                    ui.label(format!("- {} ({})", &rule.name, &rule.rule_type));
                }
            }
        });
    }

    #[inline]
    fn shift_cursor_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    #[inline]
    fn shift_cursor_down(&mut self) {
        self.cursor = self
            .cursor
            .saturating_add(1)
            .min(self.rules.len().saturating_sub(1));
    }
}
