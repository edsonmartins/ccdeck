//! Desenho da TUI (ver protótipo em docs/SPEC.md §5).

use std::sync::OnceLock;

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

use crate::app::{App, Item, Modal, Task, TaskState, View, Zone, ago};
use crate::input::Field;
use crate::model::{PERMISSION_MODES, Status, permission_mode_label};

fn no_color() -> bool {
    static NC: OnceLock<bool> = OnceLock::new();
    *NC.get_or_init(|| std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()))
}

fn c(r: u8, g: u8, b: u8) -> Color {
    if no_color() {
        Color::Reset
    } else {
        Color::Rgb(r, g, b)
    }
}

fn fg() -> Color {
    c(0xd6, 0xdb, 0xe3)
}
fn muted() -> Color {
    c(0x8b, 0x95, 0xa5)
}
fn border() -> Color {
    c(0x2a, 0x31, 0x3c)
}
fn panel() -> Color {
    c(0x15, 0x1a, 0x21)
}
fn sel_bg() -> Color {
    c(0x1f, 0x2a, 0x37)
}
fn accent() -> Color {
    c(0x5f, 0xd7, 0xaf)
}
fn collision_color() -> Color {
    c(0xff, 0xb8, 0x6b)
}
fn added_color() -> Color {
    c(0x5f, 0xd7, 0xaf)
}

pub fn status_color(s: Status) -> Color {
    match s {
        Status::Working => c(0x6c, 0xb6, 0xff),
        Status::Approval => c(0xf5, 0xa5, 0x24),
        Status::Yours => c(0xc4, 0xa7, 0xff),
        Status::Error => c(0xff, 0x7b, 0x72),
        Status::Starting | Status::Ended | Status::Suspended => muted(),
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(f.area());
    draw_header(f, app, header);
    // Este painel é a barra; a sessão está ao lado, no tmux (ADR-0018).
    // O Resumo abre com zoom e usa a janela inteira.
    if matches!(app.modal, Modal::Details) && body.width >= 80 {
        f.render_widget(Clear, body);
        draw_detail(f, app, body);
        draw_footer(f, app, footer);
        return;
    }
    if app.view == View::Summary && body.width >= 80 {
        let [side, main] =
            Layout::horizontal([Constraint::Length(42), Constraint::Fill(1)]).areas(body);
        draw_sidebar(f, app, side);
        draw_summary(f, app, main);
    } else {
        draw_ui_bar(f, app, body);
    }
    draw_footer(f, app, footer);
    draw_modal(f, app);
}

/// Barra lateral do modo integrado: árvore + cartão da sessão selecionada.
fn draw_ui_bar(f: &mut Frame, app: &App, area: Rect) {
    let card_h = if app.selected_session().is_some() {
        9
    } else {
        0
    };
    let [tree, card] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(card_h)]).areas(area);
    draw_sidebar(f, app, tree);
    let Some(s) = app.selected_session() else {
        return;
    };
    let st = app.status_of(s);
    let w = card.width.saturating_sub(2) as usize;
    let block = rounded(status_color(st)).title(Span::styled(
        format!(" {} {} ", st.icon(), st.label()),
        Style::new().fg(status_color(st)).bold(),
    ));
    let inner = block.inner(card);
    f.render_widget(block, card);
    let mut lines = vec![Line::styled(
        truncate(&branch_with_pr(app, s), w),
        Style::new().fg(fg()),
    )];
    let pending = app
        .runtime
        .get(&s.id)
        .and_then(|r| r.record.as_ref())
        .and_then(|r| r.pending.clone())
        .filter(|_| st == Status::Approval);
    if let Some(p) = pending {
        lines.push(Line::styled(
            truncate(&format!("▲ {}: {}", p.tool, p.summary), w),
            Style::new().fg(status_color(Status::Approval)),
        ));
        lines.push(Line::from(vec![
            Span::styled("y", Style::new().fg(accent()).bold()),
            Span::raw(" aprovar  "),
            Span::styled("x", Style::new().fg(status_color(Status::Error)).bold()),
            Span::raw(" recusar"),
        ]));
    } else if let Some(m) = app.last_message(s) {
        let first = m
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .to_string();
        lines.push(Line::styled(truncate(&first, w), Style::new().fg(muted())));
    }
    if let Some((c, hot)) = context_line(app, &s.id) {
        let color = if hot {
            status_color(Status::Approval)
        } else {
            muted()
        };
        lines.push(Line::styled(truncate(&c, w), Style::new().fg(color)));
    }
    if let Some(u) = compact_usage(app, &s.id) {
        lines.push(Line::styled(truncate(&u, w), Style::new().fg(muted())));
    }
    lines.push(Line::styled(
        truncate(&format!("verificação: {}", verify_text(app, s)), w),
        Style::new().fg(muted()),
    ));
    if let Some(r) = app.runtime.get(&s.id) {
        let last = r.record.as_ref().map(|x| x.at).unwrap_or(r.since);
        if let Some(m) = crate::app::stale_minutes(r.status, last, chrono::Utc::now()) {
            lines.push(Line::styled(
                truncate(&format!("{} · sem eventos há {m} min", r.event), w),
                Style::new().fg(status_color(Status::Approval)),
            ));
        }
    }
    let n = app.changes_of(s).map(Vec::len).unwrap_or(0);
    let coll = app.collisions.get(&s.id).map(|c| c.len()).unwrap_or(0);
    if n > 0 || coll > 0 {
        let mut t = format!("{n} arquivo(s) alterado(s)");
        if coll > 0 {
            t.push_str(&format!(" · ⚠ {coll} em colisão"));
        }
        t.push_str(" · i detalhes");
        let color = if coll > 0 { collision_color() } else { muted() };
        lines.push(Line::styled(truncate(&t, w), Style::new().fg(color)));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    // Sem foco, as teclas vão para a sessão da direita: isso precisa saltar aos olhos.
    if !app.focused && app.ui.is_some() {
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                " ⌨ digitando na sessão · Ctrl-q volta ",
                Style::new().fg(Color::Black).bg(accent()).bold(),
            )]))
            .bg(accent()),
            area,
        );
        return;
    }
    let total = app.state.sessions.len();
    let att = app.attention_count();
    let mut spans = vec![
        Span::styled(" ccdeck ", Style::new().fg(accent()).bold()),
        Span::styled(
            format!(" {total} {} ", plural(total, "sessão", "sessões")),
            Style::new().fg(muted()),
        ),
    ];
    if att > 0 {
        spans.push(Span::styled(
            format!(
                "· {att} {} ",
                plural(att, "precisa de você", "precisam de você")
            ),
            Style::new().fg(status_color(Status::Approval)).bold(),
        ));
    }
    let coll = app.collision_count();
    if coll > 0 {
        spans.push(Span::styled(
            format!("· ⚠ {coll} "),
            Style::new().fg(collision_color()).bold(),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).bg(panel()), area);
}

fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 { one } else { many }
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    // Ao lado da sessão o separador é o do tmux (ADR-0018); no Resumo, a borda.
    let borders = if app.view != View::Summary {
        Borders::NONE
    } else {
        Borders::RIGHT
    };
    let block = Block::new()
        .borders(borders)
        .border_style(Style::new().fg(border()));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [tabs, filter, list, buttons] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    let mut zones = app.click_zones.borrow_mut();
    zones.clear();

    let tab = |label: String, on: bool, color: Color| {
        if on {
            Span::styled(
                format!(" {label} "),
                Style::new().fg(color).bg(sel_bg()).bold(),
            )
        } else {
            Span::styled(format!(" {label} "), Style::new().fg(muted()))
        }
    };
    let tab_spans = [
        (
            tab("Projetos".into(), app.view == View::Tree, accent()),
            View::Tree,
        ),
        (
            tab(
                format!("Atenção ({})", app.attention_count()),
                app.view == View::Attention,
                status_color(Status::Approval),
            ),
            View::Attention,
        ),
        (
            tab("Resumo".into(), app.view == View::Summary, accent()),
            View::Summary,
        ),
    ];
    let mut spans = Vec::new();
    let mut x = tabs.x;
    for (span, view) in tab_spans {
        let w = span.content.chars().count() as u16;
        zones.push((
            Rect::new(x, tabs.y, w.min(tabs.right().saturating_sub(x)), 1),
            Zone::Tab(view),
        ));
        x += w + 1;
        spans.push(span);
        spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), tabs);
    zones.push((filter, Zone::Filter));

    let filtering = matches!(app.modal, Modal::Filter);
    let ftxt = if app.filter.value.is_empty() && !filtering {
        Span::styled(" / filtrar…", Style::new().fg(muted()))
    } else {
        Span::styled(
            format!(" / {}", app.filter.with_cursor(filtering)),
            Style::new().fg(fg()),
        )
    };
    f.render_widget(Paragraph::new(Line::from(ftxt)), filter);

    let items = app.items();
    let height = list.height as usize;
    let offset = app.cursor.saturating_sub(height.saturating_sub(1));
    app.list_geom.set((list, offset));
    let width = list.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    if items.is_empty() {
        let msg = match app.view {
            View::Tree if app.state.projects.is_empty() => {
                " Nenhum projeto. Clique em + Projeto (ou N)."
            }
            View::Tree | View::Summary => " Nada encontrado.",
            View::Attention => " Nenhuma sessão precisa de você.",
        };
        lines.push(Line::styled(msg, Style::new().fg(muted())));
    }
    for (pos, item) in items.iter().enumerate().skip(offset).take(height) {
        let selected = pos == app.cursor;
        let line = match *item {
            Item::Project(i) => {
                let p = &app.state.projects[i];
                let arrow = if p.collapsed { "▸" } else { "▾" };
                let n = app
                    .state
                    .sessions
                    .iter()
                    .filter(|s| s.project_id == p.id)
                    .count();
                Line::from(vec![
                    Span::styled(format!(" {arrow} "), Style::new().fg(muted())),
                    Span::styled(p.name.clone(), Style::new().fg(fg()).bold()),
                    Span::styled(format!("  {n}"), Style::new().fg(muted())),
                ])
            }
            Item::Session(i) => {
                let s = &app.state.sessions[i];
                let st = app.status_of(s);
                let col = status_color(st);
                let (indent, name) = if app.view == View::Attention {
                    let proj = app
                        .project(&s.project_id)
                        .map(|p| p.name.as_str())
                        .unwrap_or("");
                    (" ", format!("{} · {proj}", s.name))
                } else {
                    ("   ", s.name.clone())
                };
                let right = st.short();
                let warn = if app.collisions.contains_key(&s.id) {
                    "⚠ "
                } else {
                    "  "
                };
                let room =
                    width.saturating_sub(indent.chars().count() + 4 + right.chars().count() + 2);
                let name = truncate(&name, room);
                let pad = room.saturating_sub(name.chars().count());
                Line::from(vec![
                    Span::raw(indent),
                    Span::styled(format!("{} ", st.icon()), Style::new().fg(col)),
                    Span::styled(name, Style::new().fg(fg())),
                    Span::raw(" ".repeat(pad + 1)),
                    Span::styled(warn, Style::new().fg(collision_color())),
                    Span::styled(right, Style::new().fg(col)),
                ])
            }
        };
        lines.push(if selected { line.bg(sel_bg()) } else { line });
    }
    f.render_widget(Paragraph::new(lines), list);

    let button = |label: &'static str| {
        Span::styled(
            format!(" {label} "),
            Style::new().fg(fg()).bg(sel_bg()).bold(),
        )
    };
    let mut spans = Vec::new();
    let mut x = buttons.x + 1;
    spans.push(Span::raw(" "));
    for (label, zone) in [
        ("+ Sessão", Zone::NewSession),
        ("+ Projeto", Zone::NewProject),
        ("? Ajuda", Zone::Help),
    ] {
        let w = label.chars().count() as u16 + 2;
        if x + w > buttons.right() {
            break;
        }
        zones.push((Rect::new(x, buttons.y, w, 1), zone));
        x += w + 1;
        spans.push(button(label));
        spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), buttons);
}

/// Detalhes da sessão (`i`), em zoom: o painel antigo sem o preview, já que
/// a sessão está viva ao lado (ADR-0018).
fn draw_detail(f: &mut Frame, app: &App, area: Rect) {
    let area = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let Some(s) = app.selected_session() else {
        let text = match app.selected_project() {
            Some(p) => vec![
                Line::styled(p.name.clone(), Style::new().fg(fg()).bold()),
                Line::styled(p.path.display().to_string(), Style::new().fg(muted())),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("n", Style::new().fg(accent()).bold()),
                    Span::raw(" cria uma sessão neste projeto"),
                ]),
                Line::from(vec![
                    Span::styled("Enter", Style::new().fg(accent()).bold()),
                    Span::raw(" recolhe/expande"),
                ]),
            ],
            None => vec![
                Line::styled("Bem-vindo ao ccdeck", Style::new().fg(fg()).bold()),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("N", Style::new().fg(accent()).bold()),
                    Span::raw(" cadastra um projeto (diretório ou repositório git)"),
                ]),
                Line::from(vec![
                    Span::styled("n", Style::new().fg(accent()).bold()),
                    Span::raw(" cria uma sessão do Claude Code no projeto selecionado"),
                ]),
                Line::from(vec![
                    Span::styled("?", Style::new().fg(accent()).bold()),
                    Span::raw(" mostra todos os atalhos"),
                ]),
            ],
        };
        f.render_widget(Paragraph::new(text), area);
        return;
    };
    let rt = app.runtime.get(&s.id);
    let st = app.status_of(s);
    let col = status_color(st);
    let proj = app.project(&s.project_id);

    let [title, info, middle, notes] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(8),
        Constraint::Fill(1),
        Constraint::Length(3),
    ])
    .areas(area);
    draw_middle(f, app, s, middle);

    let [tl, tr] = Layout::horizontal([Constraint::Fill(1), Constraint::Length(30)]).areas(title);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(s.name.clone(), Style::new().fg(fg()).bold()),
            Span::styled(
                format!("  {}", proj.map(|p| p.name.as_str()).unwrap_or("")),
                Style::new().fg(muted()),
            ),
        ])),
        tl,
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            format!("{} {}", st.icon(), st.label()),
            Style::new().fg(col).bold(),
        ))
        .right_aligned(),
        tr,
    );

    let mut info_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border()));
    if let Some((line, hot)) = context_line(app, &s.id) {
        let color = if hot {
            status_color(Status::Approval)
        } else {
            muted()
        };
        info_block =
            info_block.title_bottom(Line::styled(format!(" {line} "), Style::new().fg(color)));
    }
    if let Some(line) = compact_usage(app, &s.id) {
        info_block = info_block.title_bottom(
            Line::styled(format!(" {line} "), Style::new().fg(muted())).right_aligned(),
        );
    }
    let ii = info_block.inner(info);
    f.render_widget(info_block, info);
    let ii = ii.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 0,
    });
    let cols = Layout::horizontal([Constraint::Ratio(1, 3); 3])
        .spacing(3)
        .split(ii);
    let col_w = cols[1].width as usize;
    let kv = |k: &str, v: String| {
        vec![
            Line::styled(k.to_string(), Style::new().fg(muted())),
            Line::styled(v, Style::new().fg(fg())),
        ]
    };
    let since = rt.map(|r| ago(r.since)).unwrap_or_else(|| "—".into());
    let event = rt
        .map(|r| {
            let last = r.record.as_ref().map(|x| x.at).unwrap_or(r.since);
            match crate::app::stale_minutes(r.status, last, chrono::Utc::now()) {
                Some(m) => format!("{} · sem eventos há {m} min", r.event),
                None => r.event.clone(),
            }
        })
        .unwrap_or_else(|| "—".into());
    let sid = s.claude_session_id.to_string();
    let wt = if s.worktree {
        short_path(&s.cwd, col_w)
    } else {
        short_path(&s.cwd, col_w.saturating_sub(15)) + " (sem worktree)"
    };
    let ports = if s.ports.is_empty() {
        "—".to_string()
    } else {
        s.ports
            .iter()
            .map(|p| format!("{}={}", p.name, p.port))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let ports_label = match s.port_slot {
        Some(n) => format!("Portas (slot {n})"),
        None => "Portas".into(),
    };
    let setup = match app.task_state(Task::Setup, s) {
        None => "—".to_string(),
        Some(TaskState::Running) => "rodando… (janela setup)".into(),
        Some(TaskState::Ok) => "ok".into(),
        Some(TaskState::Failed(c)) => format!("falhou (código {c})"),
        Some(TaskState::Interrupted) => "interrompido".into(),
    };
    let col_lines = [
        [
            kv("Branch", branch_with_pr(app, s)),
            kv(&ports_label, ports),
            kv("Sessão Claude", format!("{}…", &sid[..8])),
        ],
        [
            kv("Diretório", wt),
            kv("Setup do worktree", setup),
            kv(
                "Modo de permissão",
                permission_mode_label(&s.permission_mode).to_string(),
            ),
        ],
        [
            kv("No estado há", since),
            kv("Último evento", event),
            kv("Verificação", verify_text(app, s)),
        ],
    ];
    for (i, group) in col_lines.into_iter().enumerate() {
        let lines: Vec<Line> = group.into_iter().flatten().collect();
        f.render_widget(Paragraph::new(lines), cols[i]);
    }

    let notes_text = if s.notes.trim().is_empty() {
        "(sem notas — e para escrever)".to_string()
    } else {
        s.notes.replace('\n', " · ")
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Notas: ", Style::new().fg(muted())),
            Span::styled(notes_text, Style::new().fg(fg())),
        ]))
        .wrap(Wrap { trim: true })
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border())),
        ),
        notes,
    );
}

/// Pedido pendente + última resposta | alterações e colisões (ADR-0007, ADR-0009).
fn draw_middle(f: &mut Frame, app: &App, s: &crate::model::Session, area: Rect) {
    let [left, right] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
        .spacing(1)
        .areas(area);
    let st = app.status_of(s);
    let pending = app
        .runtime
        .get(&s.id)
        .and_then(|r| r.record.as_ref())
        .and_then(|r| r.pending.clone())
        .filter(|_| st == Status::Approval);

    let msg_area = if let Some(p) = &pending {
        let [pa, ma] = Layout::vertical([Constraint::Length(5), Constraint::Fill(1)]).areas(left);
        let amber = status_color(Status::Approval);
        let block = rounded(amber).title(Line::from(vec![
            Span::styled(" ▲ Pedido pendente ", Style::new().fg(amber).bold()),
            Span::styled(format!("{} ", p.tool), Style::new().fg(muted())),
        ]));
        f.render_widget(
            Paragraph::new(vec![
                Line::styled(p.summary.clone(), Style::new().fg(fg())),
                Line::from(vec![
                    Span::styled("y", Style::new().fg(accent()).bold()),
                    Span::raw(" aprovar   "),
                    Span::styled("x", Style::new().fg(status_color(Status::Error)).bold()),
                    Span::raw(" recusar   "),
                    Span::styled("Enter", Style::new().fg(muted()).bold()),
                    Span::styled(" entrar e decidir lá", Style::new().fg(muted())),
                ]),
            ])
            .wrap(Wrap { trim: false }),
            block.inner(pa),
        );
        f.render_widget(block, pa);
        ma
    } else {
        left
    };

    let purple = status_color(Status::Yours);
    let mut block = rounded(border()).title(Span::styled(
        " Última resposta do Claude ",
        Style::new().fg(purple).bold(),
    ));
    if matches!(st, Status::Yours | Status::Error) {
        block = block.title_bottom(
            Line::from(vec![
                Span::styled(" m", Style::new().fg(accent()).bold()),
                Span::raw(" responder "),
            ])
            .right_aligned(),
        );
    }
    let msg = app.last_message(s).unwrap_or_else(|| {
        if st == Status::Starting {
            // Diálogo de confiança na pasta não dispara hook (validacao-v0.2.md).
            "(nenhum evento do Claude Code ainda — se demorar, entre com Enter: \
             pode ser o diálogo de confiança na pasta)"
                .into()
        } else {
            "(nenhum turno concluído ainda)".into()
        }
    });
    let inner = block.inner(msg_area);
    f.render_widget(block, msg_area);
    f.render_widget(
        Paragraph::new(msg)
            .style(Style::new().fg(fg()))
            .wrap(Wrap { trim: false }),
        inner,
    );

    // Alterações
    let changes = app.changes_of(s);
    let (add, del) = changes
        .map(|v| {
            v.iter().fold((0u32, 0u32), |(a, d), c| {
                (a + c.added.unwrap_or(0), d + c.deleted.unwrap_or(0))
            })
        })
        .unwrap_or((0, 0));
    let n = changes.map(Vec::len).unwrap_or(0);
    let block = rounded(border())
        .title(Line::from(vec![
            Span::styled(" Alterações ", Style::new().fg(fg()).bold()),
            Span::styled(
                format!("vs {} ", s.base.as_deref().unwrap_or("HEAD")),
                Style::new().fg(muted()),
            ),
        ]))
        .title(
            Line::from(vec![
                Span::styled(format!(" +{add}"), Style::new().fg(added_color())),
                Span::styled(
                    format!(" −{del}"),
                    Style::new().fg(status_color(Status::Error)),
                ),
                Span::styled(format!(" {n} arq. "), Style::new().fg(muted())),
            ])
            .right_aligned(),
        )
        .title_bottom(Line::styled(" D diff completo ", Style::new().fg(muted())).right_aligned());
    let inner = block.inner(right);
    f.render_widget(block, right);

    let coll = app.collisions.get(&s.id);
    let coll_lines = coll.map(|m| m.len()).unwrap_or(0).min(2);
    let list_h = (inner.height as usize).saturating_sub(coll_lines);
    let w = inner.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    match changes {
        None => lines.push(Line::styled(
            app.scanner
                .errors
                .get(&s.id)
                .map(|e| format!("erro: {e}"))
                .unwrap_or_else(|| "calculando…".into()),
            Style::new().fg(muted()),
        )),
        Some(v) if v.is_empty() => lines.push(Line::styled(
            "nenhuma alteração ainda",
            Style::new().fg(muted()),
        )),
        Some(v) => {
            let overflow = v.len() > list_h;
            let show = if overflow {
                list_h.saturating_sub(1)
            } else {
                v.len()
            };
            for c in v.iter().take(show) {
                let collides = coll.is_some_and(|m| m.contains_key(&c.path));
                let (mark, mcol) = if collides {
                    ("⚠", collision_color())
                } else if matches!(
                    c.kind,
                    crate::git::ChangeKind::Added | crate::git::ChangeKind::Untracked
                ) {
                    (c.kind.mark(), added_color())
                } else {
                    (c.kind.mark(), status_color(Status::Working))
                };
                let a = c
                    .added
                    .map(|n| format!("+{n}"))
                    .unwrap_or_else(|| "bin".into());
                let d = c
                    .deleted
                    .filter(|n| *n > 0)
                    .map(|n| format!("−{n}"))
                    .unwrap_or_default();
                let room = w.saturating_sub(2 + 7 + 7);
                let name = truncate_left(&c.path, room);
                let pad = room.saturating_sub(name.chars().count());
                lines.push(Line::from(vec![
                    Span::styled(format!("{mark} "), Style::new().fg(mcol)),
                    Span::styled(name, Style::new().fg(fg())),
                    Span::raw(" ".repeat(pad)),
                    Span::styled(format!("{a:>7}"), Style::new().fg(added_color())),
                    Span::styled(
                        format!("{d:>7}"),
                        Style::new().fg(status_color(Status::Error)),
                    ),
                ]));
            }
            if overflow {
                lines.push(Line::styled(
                    format!("… e mais {} arquivos", v.len() - show),
                    Style::new().fg(muted()),
                ));
            }
        }
    }
    if let Some(m) = coll {
        for (file, others) in m.iter().take(2) {
            let names = others
                .iter()
                .map(|o| app.session_name(o))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(Line::styled(
                truncate(&format!("⚠ {file} também alterado por {names}"), w),
                Style::new().fg(collision_color()),
            ));
        }
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn rounded(color: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
}

fn truncate_left(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 2 {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n - (max - 1)).collect();
    format!("…{tail}")
}

/// Últimas `h` linhas não vazias do capture-pane.
fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    if let Some(fl) = &app.flash {
        let style = if fl.error {
            Style::new().fg(status_color(Status::Error)).bold()
        } else {
            Style::new().fg(accent())
        };
        f.render_widget(
            Paragraph::new(Span::styled(format!(" {}", fl.text), style)).bg(panel()),
            area,
        );
        return;
    }
    let k = |key: &'static str, label: &'static str| {
        vec![
            Span::styled(format!(" {key}"), Style::new().fg(accent()).bold()),
            Span::styled(format!(" {label} "), Style::new().fg(fg())),
        ]
    };
    let mut spans = Vec::new();
    if matches!(app.modal, Modal::Details) {
        for (key, label) in [
            ("Esc/i", "fecha"),
            ("y/x", "aprova/recusa"),
            ("m", "responde"),
            ("D", "diff"),
            ("e", "notas"),
        ] {
            spans.extend(k(key, label));
        }
        f.render_widget(Paragraph::new(Line::from(spans)).bg(panel()), area);
        return;
    }
    if app.view != View::Summary {
        for (key, label) in [
            ("Enter", "digitar"),
            ("i", "detalhes"),
            ("Tab", "aba"),
            ("q", "sair"),
        ] {
            spans.extend(k(key, label));
        }
        f.render_widget(
            Paragraph::new(Line::from(spans))
                .bg(panel())
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    if app.view == View::Summary {
        for (key, label) in [
            ("Tab", "próxima aba"),
            ("p", "período"),
            ("/", "filtrar projeto/sessão"),
            ("j/k", "sessão"),
            ("?", "ajuda"),
            ("q", "sair"),
        ] {
            spans.extend(k(key, label));
        }
        f.render_widget(Paragraph::new(Line::from(spans)).bg(panel()), area);
        return;
    }
    let sel_status = app.selected_session().map(|s| app.status_of(s));
    let mut keys: Vec<(&'static str, &'static str)> = vec![("Enter", "entrar")];
    match sel_status {
        Some(Status::Approval) => keys.extend([("y", "aprovar"), ("x", "recusar")]),
        Some(Status::Yours | Status::Error) => keys.push(("m", "responder")),
        _ => {}
    }
    if sel_status.is_some() {
        keys.push(("t", "verificar"));
        keys.push(("w", "janelas"));
        keys.push(("E", "entregar"));
        keys.push(("D", "diff"));
        keys.push(("L", "linha do tempo"));
    }
    for (key, label) in keys.into_iter().chain([
        ("n", "nova sessão"),
        ("N", "novo projeto"),
        ("a", "atenção"),
        ("e", "notas"),
        ("d", "remover"),
        ("/", "filtrar"),
        ("?", "ajuda"),
        ("q", "sair"),
    ]) {
        spans.extend(k(key, label));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).bg(panel()), area);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let [v] = Layout::vertical([Constraint::Length(h.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [r] = Layout::horizontal([Constraint::Length(w.min(area.width))])
        .flex(Flex::Center)
        .areas(v);
    r
}

fn modal_block(title: String, color: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(fg()).bold(),
        ))
        .bg(panel())
        .padding(ratatui::widgets::Padding::horizontal(1))
}

fn field_line(label: &str, value: String, focused: bool) -> Vec<Line<'static>> {
    let marker = if focused { "›" } else { " " };
    vec![
        Line::from(vec![
            Span::styled(format!("{marker} "), Style::new().fg(accent())),
            Span::styled(label.to_string(), Style::new().fg(muted())),
        ]),
        Line::styled(
            format!("  {value}"),
            if focused {
                Style::new().fg(fg()).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(fg())
            },
        ),
    ]
}

fn draw_modal(f: &mut Frame, app: &App) {
    let area = f.area();
    match &app.modal {
        Modal::None | Modal::Filter => {}
        Modal::Deliver {
            session,
            info,
            stage,
            input,
            output,
        } => {
            use crate::app::{DeliverItem, DeliverStage};
            use crate::delivery::Action;
            let name = app
                .state
                .sessions
                .iter()
                .find(|s| &s.id == session)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            let r = centered(area, 104, area.height.saturating_sub(6).min(30));
            f.render_widget(Clear, r);
            let confirming = matches!(stage, DeliverStage::Confirm(_));
            let color = if confirming {
                status_color(Status::Approval)
            } else {
                accent()
            };
            let hint = match stage {
                DeliverStage::Menu(_) => " j/k escolhe · Enter segue · Esc fecha ",
                DeliverStage::Edit(_) => " Enter revisa · Esc volta ",
                DeliverStage::Confirm(_) => " Enter executa · Esc volta ",
            };
            let block = modal_block(format!("Entrega · {name}"), color)
                .title_bottom(Line::styled(hint, Style::new().fg(muted())).right_aligned());
            let inner = block.inner(r);
            f.render_widget(block, r);
            let vw = (inner.width as usize).saturating_sub(15);
            let kvl = |k: &str, v: String| {
                Line::from(vec![
                    Span::styled(format!("{k:<14}"), Style::new().fg(muted())),
                    Span::styled(truncate_left(&v, vw), Style::new().fg(fg())),
                ])
            };
            let mut lines = vec![
                kvl("branch", format!("{} → {}", info.branch, info.base)),
                kvl(
                    "commits",
                    match (info.ahead, info.unpushed) {
                        (Some(a), Some(u)) => format!("{a} à frente da base · {u} sem push"),
                        (Some(a), None) => format!("{a} à frente da base · branch sem upstream"),
                        _ => "—".into(),
                    },
                ),
                kvl("não commitado", format!("{} arquivo(s)", info.dirty.len())),
                kvl(
                    "origin",
                    info.remote
                        .clone()
                        .unwrap_or_else(|| "— (sem remoto)".into()),
                ),
                kvl(
                    "PR",
                    match &info.pr {
                        Some(pr) => format!("{} · {} · {}", pr_badge(pr), pr.title, pr.url),
                        None => "nenhum aberto".into(),
                    },
                ),
            ];
            if let Some(e) = &info.gh_error {
                lines.push(Line::styled(
                    format!("gh: {}", truncate(e, 90)),
                    Style::new().fg(status_color(Status::Error)),
                ));
            }
            lines.push(Line::raw(""));
            match stage {
                DeliverStage::Menu(cur) => {
                    let running = app.runner.running.is_some();
                    for (i, (item, ok)) in info.items().iter().enumerate() {
                        let label = match item {
                            DeliverItem::Commit => "commit  — git add -A && git commit",
                            DeliverItem::Push => "push    — git push -u origin <branch>",
                            DeliverItem::Pr => "PR      — gh pr create",
                        };
                        let sel = i == *cur;
                        let (style, why) = match ok {
                            Ok(()) if !running => (Style::new().fg(fg()), String::new()),
                            Ok(()) => (
                                Style::new().fg(muted()),
                                "  (aguarde a ação em andamento)".into(),
                            ),
                            Err(w) => (Style::new().fg(muted()), format!("  ({w})")),
                        };
                        let line = Line::from(vec![
                            Span::styled(
                                if sel { " › " } else { "   " },
                                Style::new().fg(accent()),
                            ),
                            Span::styled(label, style),
                            Span::styled(why, Style::new().fg(muted())),
                        ]);
                        lines.push(if sel { line.bg(sel_bg()) } else { line });
                    }
                }
                DeliverStage::Edit(item) => {
                    let label = if *item == DeliverItem::Commit {
                        "Mensagem do commit"
                    } else {
                        "Título do PR"
                    };
                    lines.extend(field_line(label, input.with_cursor(true), true));
                }
                DeliverStage::Confirm(a) => {
                    let remote = info.remote.clone().unwrap_or_default();
                    // (rótulo, valor): só o valor é cortado pela esquerda.
                    let (title, detail): (String, Vec<(&str, String)>) = match a {
                        Action::Commit { message } => (
                            format!(
                                "Commitar {} arquivo(s) com \"{message}\"?",
                                info.dirty.len()
                            ),
                            info.dirty
                                .iter()
                                .take(12)
                                .map(|d| ("", d.clone()))
                                .collect(),
                        ),
                        Action::Push => (
                            format!("Enviar {} para origin?", info.branch),
                            vec![("destino: ", remote)],
                        ),
                        Action::Pr { title, .. } => (
                            format!("Abrir PR \"{title}\"?"),
                            vec![
                                ("", format!("{} → {}", info.branch, info.base)),
                                ("repositório: ", remote),
                                ("corpo: ", "notas da sessão".into()),
                            ],
                        ),
                    };
                    lines.push(Line::styled(title, Style::new().fg(color).bold()));
                    for (label, value) in detail {
                        let room = (inner.width as usize).saturating_sub(3 + label.chars().count());
                        lines.push(Line::from(vec![
                            Span::styled(format!("  {label}"), Style::new().fg(muted())),
                            Span::styled(truncate_left(&value, room), Style::new().fg(fg())),
                        ]));
                    }
                    if let Action::Commit { .. } = a
                        && info.dirty.len() > 12
                    {
                        lines.push(Line::styled(
                            format!("  … e mais {}", info.dirty.len() - 12),
                            Style::new().fg(muted()),
                        ));
                    }
                }
            }
            if let Some((ok, out)) = output {
                lines.push(Line::raw(""));
                let c = if *ok {
                    added_color()
                } else {
                    status_color(Status::Error)
                };
                for l in out
                    .lines()
                    .rev()
                    .take(6)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                {
                    lines.push(Line::styled(format!("  {l}"), Style::new().fg(c)));
                }
            }
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
        }
        Modal::Broadcast {
            text,
            targets,
            focus,
            confirming,
        } => {
            let r = centered(area, 96, targets.len() as u16 + 11);
            f.render_widget(Clear, r);
            let marked: Vec<&crate::broadcast::Target> =
                targets.iter().filter(|t| t.checked).collect();
            let color = if *confirming {
                status_color(Status::Approval)
            } else {
                accent()
            };
            let hint = if *confirming {
                " Enter envia · Esc volta "
            } else {
                " Tab/↑↓ move · Espaço marca · * todas · Enter revisa · Esc cancela "
            };
            let block = modal_block("Broadcast — sessões em Aguardando você".into(), color)
                .title_bottom(Line::styled(hint, Style::new().fg(muted())).right_aligned());
            let inner = block.inner(r);
            f.render_widget(block, r);
            let name_of = |sid: &str| {
                app.state
                    .sessions
                    .iter()
                    .find(|s| s.id == sid)
                    .map(|s| {
                        let p = app
                            .project(&s.project_id)
                            .map(|p| p.name.as_str())
                            .unwrap_or("");
                        (s.name.clone(), p.to_string())
                    })
                    .unwrap_or_default()
            };
            let mut lines = field_line(
                "Texto (enviado como no m, seguido de Enter)",
                text.with_cursor(*focus == 0 && !*confirming),
                *focus == 0,
            );
            lines.push(Line::raw(""));
            if *confirming {
                lines.push(Line::styled(
                    format!(
                        "Enviar \"{}\" para {} sessões?",
                        text.value.trim(),
                        marked.len()
                    ),
                    Style::new().fg(color).bold(),
                ));
                for t in &marked {
                    let (n, p) = name_of(&t.session);
                    lines.push(Line::styled(format!("  ◆ {n}  {p}"), Style::new().fg(fg())));
                }
                lines.push(Line::styled(
                    "Cada uma é conferida de novo no envio; quem mudou de estado é pulada.",
                    Style::new().fg(muted()),
                ));
            } else {
                lines.push(Line::styled(
                    format!("Destinos ({} de {} marcadas)", marked.len(), targets.len()),
                    Style::new().fg(muted()),
                ));
                for (i, t) in targets.iter().enumerate() {
                    let (n, p) = name_of(&t.session);
                    let sel = *focus == i + 1;
                    let line = Line::from(vec![
                        Span::styled(if sel { " › " } else { "   " }, Style::new().fg(accent())),
                        Span::styled(
                            if t.checked { "[x] " } else { "[ ] " },
                            Style::new().fg(accent()),
                        ),
                        Span::styled(format!("{n:<28}"), Style::new().fg(fg())),
                        Span::styled(p, Style::new().fg(muted())),
                    ]);
                    lines.push(if sel { line.bg(sel_bg()) } else { line });
                }
            }
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
        }
        Modal::Windows {
            session,
            items,
            cursor,
            confirm,
        } => {
            use crate::windows::Kind;
            let name = app
                .state
                .sessions
                .iter()
                .find(|s| &s.id == session)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            let r = centered(area, 96, items.len() as u16 + 6);
            f.render_widget(Clear, r);
            let block = modal_block(format!("Janelas · {name}"), accent()).title_bottom(
                Line::styled(
                    " Enter abre/entra · K fecha · Ctrl-q volta ao ccdeck · Esc ",
                    Style::new().fg(muted()),
                )
                .right_aligned(),
            );
            let inner = block.inner(r);
            f.render_widget(block, r);
            let mut lines: Vec<Line> = items
                .iter()
                .enumerate()
                .map(|(i, it)| {
                    let sel = i == *cursor;
                    let mark = if it.open { "●" } else { "○" };
                    let kind = match it.kind {
                        Kind::Claude => "Claude Code",
                        Kind::Shell => "shell no worktree",
                        Kind::Project => ".ccdeck.json",
                        Kind::Other => "existente",
                    };
                    let detail = it.command.clone().unwrap_or_default();
                    let warn = confirm.as_deref() == Some(it.name.as_str());
                    let mut spans = vec![
                        Span::styled(if sel { " › " } else { "   " }, Style::new().fg(accent())),
                        Span::styled(
                            format!("{mark} "),
                            Style::new().fg(if it.open { added_color() } else { muted() }),
                        ),
                        Span::styled(format!("{:<16}", it.name), Style::new().fg(fg()).bold()),
                        Span::styled(format!("{kind:<20}"), Style::new().fg(muted())),
                        Span::styled(truncate(&detail, 30), Style::new().fg(muted())),
                    ];
                    if warn {
                        spans.push(Span::styled(
                            "  K de novo fecha",
                            Style::new().fg(status_color(Status::Error)).bold(),
                        ));
                    }
                    let line = Line::from(spans);
                    if sel { line.bg(sel_bg()) } else { line }
                })
                .collect();
            lines.insert(
                0,
                Line::styled(
                    " ● aberta  ○ será criada no worktree, com o ambiente da sessão",
                    Style::new().fg(muted()),
                ),
            );
            lines.insert(1, Line::raw(""));
            f.render_widget(Paragraph::new(lines), inner);
        }
        Modal::Timeline {
            name,
            entries,
            scroll,
        } => {
            use crate::timeline::Kind;
            let r = centered(area, 110, area.height.saturating_sub(4));
            f.render_widget(Clear, r);
            let block = modal_block(format!("Linha do tempo · {name}"), accent()).title_bottom(
                Line::styled(
                    " j/k rolar · Esc fecha · horários locais ",
                    Style::new().fg(muted()),
                )
                .right_aligned(),
            );
            let inner = block.inner(r);
            f.render_widget(block, r);
            let lines: Vec<Line> = entries
                .iter()
                .skip(*scroll)
                .take(inner.height as usize)
                .map(|e| {
                    let when =
                        e.at.with_timezone(&chrono::Local)
                            .format("%d/%m %H:%M:%S  ")
                            .to_string();
                    let mut spans = vec![Span::styled(when, Style::new().fg(muted()))];
                    match &e.kind {
                        Kind::State(st, dur) => {
                            spans.push(Span::styled(
                                format!("{} {:<14}", st.icon(), st.short()),
                                Style::new().fg(status_color(*st)),
                            ));
                            spans.push(Span::styled(
                                format!("{:<12}", dur.map(fmt_secs).unwrap_or_default()),
                                Style::new().fg(muted()),
                            ));
                            spans.push(Span::styled(e.text.clone(), Style::new().fg(fg())));
                        }
                        Kind::Action => {
                            spans.push(Span::styled(
                                format!("{:<28}", "⇢ ação do ccdeck"),
                                Style::new().fg(accent()).bold(),
                            ));
                            spans.push(Span::styled(e.text.clone(), Style::new().fg(accent())));
                        }
                        Kind::Compaction => {
                            spans.push(Span::styled(
                                format!("{:<28}", "⟲ contexto"),
                                Style::new().fg(status_color(Status::Approval)).bold(),
                            ));
                            spans.push(Span::styled(e.text.clone(), Style::new().fg(fg())));
                        }
                    }
                    Line::from(spans)
                })
                .collect();
            f.render_widget(Paragraph::new(lines), inner);
        }
        Modal::Details => {}
        Modal::Help => {
            let r = centered(area, 70, 32);
            f.render_widget(Clear, r);
            let rows = [
                (
                    "clique",
                    "seleciona na árvore; no painel da direita, digita nele",
                ),
                ("j/k ↑/↓", "navegar"),
                ("h/l ←/→", "recolher / expandir projeto"),
                (
                    "< / >",
                    "estreitar / alargar a barra (ou arraste a divisória)",
                ),
                ("Enter", "digitar na sessão (retoma se suspensa)"),
                ("Ctrl-q", "na sessão: volta à árvore; na árvore: desconecta"),
                ("y / x", "aprovar / recusar o pedido pendente"),
                ("m", "responder sem entrar na sessão"),
                ("D", "diff completo da sessão"),
                ("i", "detalhes da sessão (resposta, alterações, colisões)"),
                ("L", "linha do tempo da sessão"),
                ("t / T", "verificar (verify do .ccdeck.json) / ver o log"),
                ("w", "janelas da sessão (shell, servidor, logs)"),
                ("B", "broadcast: mesmo texto para várias sessões"),
                ("E", "entrega: commit, push e PR (gh), com confirmação"),
                ("n / N", "nova sessão / novo projeto"),
                ("a", "alternar visão Atenção"),
                ("Tab", "Projetos → Atenção → Resumo"),
                ("p", "período do Resumo (hoje/7/30 dias)"),
                ("/", "filtrar (Esc limpa)"),
                ("e", "editar notas (Ctrl-s salva)"),
                ("r", "renomear sessão"),
                ("d", "remover sessão ou projeto vazio"),
                ("q", "sair — as sessões continuam rodando"),
            ];
            let mut lines: Vec<Line> = rows
                .iter()
                .map(|(k, v)| {
                    Line::from(vec![
                        Span::styled(format!("{k:<10}"), Style::new().fg(accent()).bold()),
                        Span::styled(*v, Style::new().fg(fg())),
                    ])
                })
                .collect();
            let mut legend = vec![Span::raw(" ")];
            for st in [
                Status::Working,
                Status::Approval,
                Status::Yours,
                Status::Error,
                Status::Suspended,
            ] {
                legend.push(Span::styled(
                    format!("{} {}  ", st.icon(), st.short()),
                    Style::new().fg(status_color(st)),
                ));
            }
            legend.push(Span::styled(
                "⚠ colisão",
                Style::new().fg(collision_color()),
            ));
            lines.insert(0, Line::from(legend));
            lines.insert(1, Line::raw(""));
            f.render_widget(
                Paragraph::new(lines).block(modal_block("Atalhos".into(), accent())),
                r,
            );
        }
        Modal::Reply { session, text } => {
            let Some(s) = app.state.sessions.iter().find(|s| &s.id == session) else {
                return;
            };
            let r = centered(area, 80, 12);
            f.render_widget(Clear, r);
            let last = app.last_message(s).unwrap_or_default();
            let mut lines: Vec<Line> = last
                .lines()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|l| Line::styled(l.to_string(), Style::new().fg(muted())))
                .collect();
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "Sua resposta (enviada como se você digitasse na sessão):",
                Style::new().fg(muted()),
            ));
            lines.push(Line::styled(
                text.with_cursor(true),
                Style::new().fg(fg()).bold(),
            ));
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "Enter envia · Esc cancela",
                Style::new().fg(muted()),
            ));
            f.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .block(modal_block(
                        format!("Responder a {}", s.name),
                        status_color(Status::Yours),
                    )),
                r,
            );
        }
        Modal::NewProject(t) => {
            let r = centered(area, 72, 7);
            f.render_widget(Clear, r);
            f.render_widget(
                Paragraph::new(vec![
                    Line::styled(
                        "Diretório do projeto (raiz do repositório git, de preferência)",
                        Style::new().fg(muted()),
                    ),
                    Line::styled(t.with_cursor(true), Style::new().fg(fg()).bold()),
                    Line::raw(""),
                    Line::styled(
                        "Enter cadastra · Esc cancela · Ctrl-u limpa",
                        Style::new().fg(muted()),
                    ),
                ])
                .block(modal_block("Novo projeto".into(), accent())),
                r,
            );
        }
        Modal::NewSession(form) => {
            let prep_h = if form.worktree || form.prep.iter().any(|l| l.starts_with("portas")) {
                form.prep.len() as u16 + 1
            } else {
                0
            };
            let starter_h = if form.starters.is_empty() {
                0
            } else {
                2 + u16::from(form.current_starter().is_some_and(|s| s.warning.is_some()))
            };
            let r = centered(area, 96, 15 + prep_h + starter_h);
            f.render_widget(Clear, r);
            let foc = form.focused();
            let slug = crate::model::slugify(if form.name.value.trim().is_empty() {
                "nome"
            } else {
                &form.name.value
            });
            let wt = if !form.is_repo {
                "[ ] não é um repositório git — roda no diretório do projeto".to_string()
            } else if form.worktree {
                format!(
                    "[x] criar worktree → branch {}{slug}",
                    app.config.branch_prefix
                )
            } else {
                "[ ] rodar direto no repositório (sem isolamento)".to_string()
            };
            let modes = PERMISSION_MODES
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let m = permission_mode_label(m);
                    if i == form.mode {
                        format!("(•) {m}")
                    } else {
                        format!("( ) {m}")
                    }
                })
                .collect::<Vec<_>>()
                .join("  ");
            let mut lines = Vec::new();
            if !form.starters.is_empty() {
                let label = form
                    .current_starter()
                    .map(|s| s.label.clone())
                    .unwrap_or_else(|| format!("(nenhum) — {} opções", form.starters.len()));
                lines.extend(field_line(
                    "Modelo (←/→: templates e changes do OpenSpec)",
                    label,
                    foc == Field::Starter,
                ));
                if let Some(w) = form.current_starter().and_then(|s| s.warning.clone()) {
                    lines.push(Line::styled(
                        format!("  {w}"),
                        Style::new().fg(status_color(Status::Approval)),
                    ));
                }
            }
            lines.extend(field_line(
                "Nome",
                form.name.with_cursor(foc == Field::Name),
                foc == Field::Name,
            ));
            lines.extend(field_line(
                "Branch base",
                form.base.with_cursor(foc == Field::Base),
                foc == Field::Base,
            ));
            lines.extend(field_line(
                "Isolamento (Espaço alterna)",
                wt,
                foc == Field::Worktree,
            ));
            if prep_h > 0 {
                lines.push(Line::styled(
                    "  Preparo do worktree (.ccdeck.json)",
                    Style::new().fg(muted()),
                ));
                for l in &form.prep {
                    let applies = form.worktree || l.starts_with("portas") || l.starts_with("env");
                    lines.push(Line::styled(
                        format!("    {l}"),
                        Style::new().fg(if applies { accent() } else { muted() }),
                    ));
                }
            }
            lines.extend(field_line(
                "Modo de permissão (←/→)",
                modes,
                foc == Field::Mode,
            ));
            lines.extend(field_line(
                "Prompt inicial (opcional)",
                form.prompt.with_cursor(foc == Field::Prompt),
                foc == Field::Prompt,
            ));
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "Tab próximo campo · Enter criar e entrar · Esc cancelar",
                Style::new().fg(muted()),
            ));
            f.render_widget(
                Paragraph::new(lines).block(modal_block(
                    format!("Nova sessão em {}", form.project_name),
                    accent(),
                )),
                r,
            );
        }
        Modal::Delete { session, dirty } => {
            let Some(s) = app.state.sessions.iter().find(|s| &s.id == session) else {
                return;
            };
            let r = centered(area, 72, 11);
            f.render_widget(Clear, r);
            let red = status_color(Status::Error);
            let mut lines = vec![Line::styled(
                format!("Diretório: {}", short_path(&s.cwd, 60)),
                Style::new().fg(muted()),
            )];
            if *dirty > 0 {
                lines.push(Line::styled(
                    format!("{dirty} arquivo(s) com alterações não commitadas."),
                    Style::new().fg(status_color(Status::Approval)),
                ));
            }
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("1", Style::new().fg(accent()).bold()),
                Span::raw(" só encerrar a sessão (mantém worktree e branch)"),
            ]));
            if s.worktree {
                let mut l2 = vec![
                    Span::styled("2", Style::new().fg(accent()).bold()),
                    Span::raw(" encerrar e remover worktree"),
                ];
                if *dirty > 0 {
                    l2.push(Span::styled(
                        " — bloqueado: há alterações",
                        Style::new().fg(red),
                    ));
                }
                lines.push(Line::from(l2));
                lines.push(Line::from(vec![
                    Span::styled("3", Style::new().fg(accent()).bold()),
                    Span::raw(" encerrar e forçar remoção "),
                    Span::styled("(perde as alterações)", Style::new().fg(red)),
                ]));
            }
            lines.push(Line::styled(
                "O branch nunca é apagado automaticamente. Esc cancela.",
                Style::new().fg(muted()),
            ));
            f.render_widget(
                Paragraph::new(lines).block(modal_block(format!("Remover \"{}\"?", s.name), red)),
                r,
            );
        }
        Modal::Notes { session, text } => {
            let name = app
                .state
                .sessions
                .iter()
                .find(|s| &s.id == session)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            let r = centered(area, 76, 14);
            f.render_widget(Clear, r);
            let mut lines: Vec<Line> = text
                .with_cursor(true)
                .lines()
                .map(|l| Line::styled(l.to_string(), Style::new().fg(fg())))
                .collect();
            if text.value.ends_with('\n') {
                lines.push(Line::raw(""));
            }
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "Ctrl-s salva · Esc cancela",
                Style::new().fg(muted()),
            ));
            f.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .block(modal_block(format!("Notas — {name}"), accent())),
                r,
            );
        }
        Modal::Rename { text, .. } => {
            let r = centered(area, 60, 6);
            f.render_widget(Clear, r);
            f.render_widget(
                Paragraph::new(vec![
                    Line::styled(text.with_cursor(true), Style::new().fg(fg()).bold()),
                    Line::raw(""),
                    Line::styled(
                        "Enter salva · Esc cancela (branch e worktree não mudam)",
                        Style::new().fg(muted()),
                    ),
                ])
                .block(modal_block("Renomear sessão".into(), accent())),
                r,
            );
        }
    }
}

/// `~` no lugar do HOME e reticências à esquerda quando não cabe.
fn short_path(p: &std::path::Path, max: usize) -> String {
    let mut s = p.display().to_string();
    if let Some(home) = std::env::var_os("HOME").map(|h| h.to_string_lossy().to_string())
        && !home.is_empty()
        && s.starts_with(&home)
    {
        s = format!("~{}", &s[home.len()..]);
    }
    let n = s.chars().count();
    if n <= max || max < 2 {
        return s;
    }
    let tail: String = s.chars().skip(n - (max - 1)).collect();
    format!("…{tail}")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

// --- Resumo (ADR-0010) -------------------------------------------------------

/// `1,2M`, `45,3k`, `812`.
fn fmt_tokens(n: u64) -> String {
    let f = |v: f64, suf: &str| format!("{v:.1}{suf}").replace('.', ",");
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => f(n as f64 / 1e3, "k"),
        1_000_000..1_000_000_000 => f(n as f64 / 1e6, "M"),
        _ => f(n as f64 / 1e9, "B"),
    }
}

fn fmt_usd(c: &crate::summary::Cost) -> String {
    let v = format!("US$ {:.2}", c.usd).replace('.', ",");
    if c.unknown { format!("{v} + ?") } else { v }
}

fn fmt_dur(d: chrono::Duration) -> String {
    let m = d.num_minutes();
    match m {
        ..1 => "0 min".into(),
        1..60 => format!("{m} min"),
        _ => format!("{} h {:02} min", m / 60, m % 60),
    }
}

fn show_cost(app: &App) -> bool {
    app.config.cost_display == crate::model::CostDisplay::Api
}

/// Linha do painel da sessão: tokens, custo e espera de hoje.
fn compact_usage(app: &App, sid: &str) -> Option<String> {
    let row = app
        .summary_today
        .as_ref()?
        .rows
        .iter()
        .find(|r| r.session == sid)?;
    let tok = row.tokens.prompt() + row.tokens.output;
    if tok == 0 && row.waiting.num_minutes() == 0 {
        return None;
    }
    let mut parts = vec![format!("hoje {} tokens", fmt_tokens(tok))];
    if show_cost(app) {
        parts.push(format!("{} (equiv. API)", fmt_usd(&row.cost)));
    }
    parts.push(format!("espera {}", fmt_dur(row.waiting)));
    Some(parts.join(" · "))
}

/// `PR #12 ✓`, `PR #12 ✕ 2`, `PR #12 … 1` (ADR-0017).
fn pr_badge(pr: &crate::delivery::Pr) -> String {
    use crate::delivery::Ci;
    match pr.ci {
        Ci::NoChecks => format!("PR #{}", pr.number),
        Ci::Ok => format!("PR #{} ✓", pr.number),
        Ci::Pending(n) => format!("PR #{} … {n}", pr.number),
        Ci::Failing(n) => format!("PR #{} ✕ {n}", pr.number),
    }
}

fn branch_with_pr(app: &App, s: &crate::model::Session) -> String {
    let b = s.branch.clone().unwrap_or_else(|| "—".into());
    match app.pr_of(s) {
        Some(pr) => format!("{b} · {}", pr_badge(pr)),
        None => b,
    }
}

/// Estado da verificação no painel (ADR-0013).
fn verify_text(app: &App, s: &crate::model::Session) -> String {
    let files = app.task_files(Task::Verify, &s.id);
    let dur = || files.duration().map(fmt_secs).unwrap_or_default();
    match app.task_state(Task::Verify, s) {
        None => "— t verifica".into(),
        Some(TaskState::Running) => format!("rodando… {} (janela verify)", dur()),
        Some(TaskState::Ok) => format!("ok ({}) · T log", dur()),
        Some(TaskState::Failed(c)) => format!("falhou (código {c}) · T log"),
        Some(TaskState::Interrupted) => "interrompida · T log".into(),
    }
}

/// `contexto 45% (452,0k / 1M) · 2 compactações` e se passou de 85% (ADR-0012).
fn context_line(app: &App, sid: &str) -> Option<(String, bool)> {
    let ins = app.usage.insight.get(sid)?;
    let ctx = ins.context.as_ref()?;
    let mut out = match crate::usage::window_for(&app.config.context_windows, &ctx.model) {
        Some(w) if w > 0 => {
            let pct = ctx.prompt as f64 * 100.0 / w as f64;
            (
                format!(
                    "contexto {pct:.0}% ({} / {})",
                    fmt_tokens(ctx.prompt),
                    fmt_window(w)
                ),
                pct >= 85.0,
            )
        }
        _ => (format!("contexto {}", fmt_tokens(ctx.prompt)), false),
    };
    match ins.compactions.len() {
        0 => {}
        1 => out.0.push_str(" · 1 compactação"),
        n => out.0.push_str(&format!(" · {n} compactações")),
    }
    Some(out)
}

/// `45 s`, `12 min 05 s`, `3 h 12 min`.
fn fmt_secs(d: chrono::Duration) -> String {
    let s = d.num_seconds();
    match s {
        ..60 => format!("{s} s"),
        60..3600 => format!("{} min {:02} s", s / 60, s % 60),
        _ => fmt_dur(d),
    }
}

/// `1M`, `200k`.
fn fmt_window(w: u64) -> String {
    if w.is_multiple_of(1_000_000) {
        format!("{}M", w / 1_000_000)
    } else {
        format!("{}k", w / 1_000)
    }
}

fn tile(f: &mut Frame, area: Rect, label: &str, value: String, sub: String, color: Color) {
    let block = rounded(border());
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(vec![
            Line::styled(label.to_string(), Style::new().fg(muted())),
            Line::styled(value, Style::new().fg(color).bold()),
            Line::styled(sub, Style::new().fg(muted())),
        ]),
        inner,
    );
}

fn draw_summary(f: &mut Frame, app: &App, area: Rect) {
    use ratatui::symbols::Marker;
    use ratatui::widgets::{
        Axis, Bar, BarChart, BarGroup, Cell, Chart, Dataset, GraphType, Row, Table,
    };

    let area = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let Some(sm) = app.summary.as_ref() else {
        f.render_widget(Paragraph::new("calculando…").fg(muted()), area);
        return;
    };
    let cost_on = show_cost(app);
    let [title, tiles, charts, table, bottom] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(11),
        Constraint::Fill(1),
        Constraint::Length(9),
    ])
    .areas(area);

    let mut tl = vec![
        Span::styled("Resumo", Style::new().fg(fg()).bold()),
        Span::styled(
            format!("  {}", sm.period.label()),
            Style::new().fg(accent()).bold(),
        ),
    ];
    if !app.filter.value.trim().is_empty() {
        tl.push(Span::styled(
            format!("  filtro: {}", app.filter.value.trim()),
            Style::new().fg(muted()),
        ));
    }
    if cost_on {
        tl.push(Span::styled(
            "  · custo = equivalente em API, não o valor pago na assinatura",
            Style::new().fg(muted()),
        ));
    }
    if app.usage.busy() {
        tl.push(Span::styled("  · indexando…", Style::new().fg(muted())));
    }
    f.render_widget(Paragraph::new(Line::from(tl)), title);

    // Blocos de números
    let n = if cost_on { 6 } else { 5 };
    let cells = Layout::horizontal(vec![Constraint::Ratio(1, n); n as usize])
        .spacing(1)
        .split(tiles);
    let t = &sm.tokens;
    let mut i = 0;
    let mut next = || {
        i += 1;
        cells[i - 1]
    };
    tile(
        f,
        next(),
        "Tokens",
        fmt_tokens(t.prompt() + t.output),
        format!("{} mensagens", t.messages),
        fg(),
    );
    if cost_on {
        let sub = if sm.unpriced.is_empty() {
            "equivalente em API".to_string()
        } else {
            format!("? sem preço: {}", sm.unpriced.join(", "))
        };
        tile(f, next(), "Custo", fmt_usd(&sm.cost), sub, accent());
    }
    tile(
        f,
        next(),
        "Cache",
        sm.cache_rate()
            .map(|r| format!("{:.0}% lido", r * 100.0))
            .unwrap_or_else(|| "—".into()),
        format!(
            "esc. {} · lê {}",
            fmt_tokens(t.cache_write()),
            fmt_tokens(t.cache_read)
        ),
        fg(),
    );
    tile(
        f,
        next(),
        "Entrada / saída",
        format!("{} / {}", fmt_tokens(t.input), fmt_tokens(t.output)),
        "sem cache".into(),
        fg(),
    );
    tile(
        f,
        next(),
        "Arquivos modificados",
        sm.files.len().to_string(),
        format!("em {} sessões", sm.rows.len()),
        fg(),
    );
    tile(
        f,
        next(),
        "Esperando você",
        fmt_dur(sm.waiting),
        "aprovação + sua vez".into(),
        status_color(Status::Yours),
    );

    // Gráficos
    let [left, right] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
        .spacing(1)
        .areas(charts);
    let unit = if sm.period.hourly() { "hora" } else { "dia" };
    let block = rounded(border()).title(Span::styled(
        format!(" Tokens por {unit} "),
        Style::new().fg(fg()).bold(),
    ));
    let inner = block.inner(left);
    f.render_widget(block, left);
    let nb = sm.buckets.len().max(1) as u16;
    let bar_w = (inner.width.saturating_sub(nb) / nb).max(1);
    let bars: Vec<Bar> = sm
        .buckets
        .iter()
        .map(|b| {
            let label = if sm.period.hourly() {
                b.start.format("%H").to_string()
            } else {
                b.start.format("%d").to_string()
            };
            Bar::default()
                .value(b.tokens)
                .text_value(String::new())
                .label(Line::from(label))
                .style(Style::new().fg(accent()))
        })
        .collect();
    f.render_widget(
        BarChart::default()
            .data(BarGroup::default().bars(&bars))
            .bar_width(bar_w)
            .bar_gap(1)
            .label_style(Style::new().fg(muted())),
        inner,
    );

    let max_cost = sm.buckets.iter().map(|b| b.cost).fold(0.0, f64::max);
    let max_msg = sm.buckets.iter().map(|b| b.messages).max().unwrap_or(0);
    let norm = |v: f64, m: f64| if m > 0.0 { v / m } else { 0.0 };
    let cost_pts: Vec<(f64, f64)> = sm
        .buckets
        .iter()
        .enumerate()
        .map(|(i, b)| (i as f64, norm(b.cost, max_cost)))
        .collect();
    let msg_pts: Vec<(f64, f64)> = sm
        .buckets
        .iter()
        .enumerate()
        .map(|(i, b)| (i as f64, norm(b.messages as f64, max_msg as f64)))
        .collect();
    let mut sets = vec![
        Dataset::default()
            .name(format!("mensagens (máx {max_msg}/{unit})"))
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(status_color(Status::Yours)))
            .data(&msg_pts),
    ];
    if cost_on {
        sets.push(
            Dataset::default()
                .name(format!("custo (máx US$ {max_cost:.2}/{unit})").replace('.', ","))
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::new().fg(accent()))
                .data(&cost_pts),
        );
    }
    let title = if cost_on {
        format!(" Custo e mensagens por {unit} (escala relativa) ")
    } else {
        format!(" Mensagens por {unit} (escala relativa) ")
    };
    let x_max = (sm.buckets.len().saturating_sub(1)).max(1) as f64;
    let first = sm
        .buckets
        .first()
        .map(|b| {
            b.start
                .format(if sm.period.hourly() { "%H:00" } else { "%d/%m" })
                .to_string()
        })
        .unwrap_or_default();
    let last = sm
        .buckets
        .last()
        .map(|b| {
            b.start
                .format(if sm.period.hourly() { "%H:00" } else { "%d/%m" })
                .to_string()
        })
        .unwrap_or_default();
    f.render_widget(
        Chart::new(sets)
            .block(rounded(border()).title(Span::styled(title, Style::new().fg(fg()).bold())))
            .x_axis(
                Axis::default()
                    .bounds([0.0, x_max])
                    .labels([first, last])
                    .style(Style::new().fg(muted())),
            )
            .y_axis(
                Axis::default()
                    .bounds([0.0, 1.05])
                    .style(Style::new().fg(muted())),
            )
            .legend_position(Some(ratatui::widgets::LegendPosition::TopLeft))
            .hidden_legend_constraints((Constraint::Percentage(100), Constraint::Percentage(100))),
        right,
    );

    // Tabela por sessão
    let mut header = vec!["Sessão", "Projeto", "Estado", "Tokens", "Cache"];
    if cost_on {
        header.push("Custo");
    }
    header.push("Espera");
    let rows: Vec<Row> = sm
        .rows
        .iter()
        .map(|r| {
            let p = r.tokens.prompt();
            let cache = if p > 0 {
                format!("{:.0}%", r.tokens.cache_read as f64 * 100.0 / p as f64)
            } else {
                "—".into()
            };
            let mut cells = vec![
                Cell::from(r.name.clone()),
                Cell::from(r.project.clone()).style(Style::new().fg(muted())),
                Cell::from(format!("{} {}", r.status.icon(), r.status.short()))
                    .style(Style::new().fg(status_color(r.status))),
                Cell::from(fmt_tokens(p + r.tokens.output)),
                Cell::from(cache),
            ];
            if cost_on {
                cells.push(Cell::from(fmt_usd(&r.cost)));
            }
            cells.push(Cell::from(fmt_dur(r.waiting)));
            Row::new(cells)
        })
        .collect();
    let mut widths = vec![
        Constraint::Fill(2),
        Constraint::Fill(1),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(6),
    ];
    if cost_on {
        widths.push(Constraint::Length(14));
    }
    widths.push(Constraint::Length(12));
    f.render_widget(
        Table::new(rows, widths)
            .header(Row::new(header).style(Style::new().fg(muted()).bold()))
            .column_spacing(2)
            .block(
                rounded(border()).title(Span::styled(" Por sessão ", Style::new().fg(fg()).bold())),
            ),
        table,
    );

    // Atividade recente | arquivos modificados
    let [act, files] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
        .spacing(1)
        .areas(bottom);
    let block = rounded(border()).title(Span::styled(
        " Atividade recente ",
        Style::new().fg(fg()).bold(),
    ));
    let inner = block.inner(act);
    f.render_widget(block, act);
    let lines: Vec<Line> = sm
        .activity
        .iter()
        .take(inner.height as usize)
        .map(|a| {
            Line::from(vec![
                Span::styled(
                    a.at.with_timezone(&chrono::Local)
                        .format("%d/%m %H:%M ")
                        .to_string(),
                    Style::new().fg(muted()),
                ),
                Span::styled(
                    format!("{} ", a.status.icon()),
                    Style::new().fg(status_color(a.status)),
                ),
                Span::styled(format!("{} ", a.session), Style::new().fg(fg())),
                Span::styled(a.event.clone(), Style::new().fg(muted())),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);

    let block = rounded(border()).title(Span::styled(
        " Arquivos modificados ",
        Style::new().fg(fg()).bold(),
    ));
    let inner = block.inner(files);
    f.render_widget(block, files);
    let w = inner.width as usize;
    let lines: Vec<Line> = sm
        .files
        .iter()
        .take(inner.height as usize)
        .map(|(path, sess, a, d)| {
            let tail = format!(" +{a} −{d}  {sess}");
            Line::from(vec![
                Span::styled(
                    truncate_left(path, w.saturating_sub(tail.chars().count())),
                    Style::new().fg(fg()),
                ),
                Span::styled(format!(" +{a}"), Style::new().fg(added_color())),
                Span::styled(
                    format!(" −{d}"),
                    Style::new().fg(status_color(Status::Error)),
                ),
                Span::styled(format!("  {sess}"), Style::new().fg(muted())),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatos() {
        assert_eq!(fmt_tokens(812), "812");
        assert_eq!(fmt_tokens(45_300), "45,3k");
        assert_eq!(fmt_tokens(1_250_000), "1,2M");
        assert_eq!(fmt_dur(chrono::Duration::minutes(29)), "29 min");
        assert_eq!(fmt_dur(chrono::Duration::minutes(192)), "3 h 12 min");
        let c = crate::summary::Cost {
            usd: 3.4,
            unknown: true,
        };
        assert_eq!(fmt_usd(&c), "US$ 3,40 + ?");
        assert_eq!(fmt_window(1_000_000), "1M");
        assert_eq!(fmt_secs(chrono::Duration::seconds(82)), "1 min 22 s");
        assert_eq!(fmt_secs(chrono::Duration::seconds(9)), "9 s");
        assert_eq!(fmt_window(200_000), "200k");
    }
}
