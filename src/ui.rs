//! Desenho da TUI (ver protótipo em docs/SPEC.md §5).

use std::sync::OnceLock;

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

use crate::app::{App, Item, Modal, SetupState, View, ago};
use crate::input::Field;
use crate::model::{PERMISSION_MODES, Status};

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
    let [side, main] =
        Layout::horizontal([Constraint::Length(42), Constraint::Fill(1)]).areas(body);
    draw_sidebar(f, app, side);
    draw_detail(f, app, main);
    draw_footer(f, app, footer);
    draw_modal(f, app);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let total = app.state.sessions.len();
    let att = app.attention_count();
    let mut spans = vec![
        Span::styled(" ccdeck ", Style::new().fg(accent()).bold()),
        Span::styled(
            format!(" {total} sessões em {} projetos ", app.state.projects.len()),
            Style::new().fg(muted()),
        ),
    ];
    if att > 0 {
        spans.push(Span::styled(
            format!(" {att} precisam de você "),
            Style::new().fg(status_color(Status::Approval)).bold(),
        ));
    }
    let coll = app.collision_count();
    if coll > 0 {
        spans.push(Span::styled(
            format!(" ⚠ {coll} sessões com colisão "),
            Style::new().fg(collision_color()).bold(),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).bg(panel()), area);
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::new()
        .borders(Borders::RIGHT)
        .border_style(Style::new().fg(border()));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [tabs, filter, list, legend] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .areas(inner);

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
    f.render_widget(
        Paragraph::new(Line::from(vec![
            tab("Projetos".into(), app.view == View::Tree, accent()),
            Span::raw(" "),
            tab(
                format!("Atenção ({})", app.attention_count()),
                app.view == View::Attention,
                status_color(Status::Approval),
            ),
        ])),
        tabs,
    );

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
    let width = list.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    if items.is_empty() {
        let msg = match app.view {
            View::Tree if app.state.projects.is_empty() => " Nenhum projeto. N cadastra um.",
            View::Tree => " Nada encontrado.",
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

    let legend_line = |s: Status, t: &'static str| {
        Span::styled(
            format!("{} {t}  ", s.icon()),
            Style::new().fg(status_color(s)),
        )
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::raw(" "),
                legend_line(Status::Working, "trabalhando"),
                legend_line(Status::Approval, "aprovação"),
            ]),
            Line::from(vec![
                Span::raw(" "),
                legend_line(Status::Yours, "sua vez"),
                legend_line(Status::Error, "erro"),
                legend_line(Status::Suspended, "suspensa"),
                Span::styled("⚠ colisão", Style::new().fg(collision_color())),
            ]),
        ]),
        legend,
    );
}

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

    let [title, info, middle, preview, notes] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(8),
        Constraint::Length(11),
        Constraint::Min(4),
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

    let info_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border()));
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
    let event = rt.map(|r| r.event.clone()).unwrap_or_else(|| "—".into());
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
    let setup = match app.setup_state(s) {
        None => "—".to_string(),
        Some(SetupState::Running) => "rodando… (janela setup)".into(),
        Some(SetupState::Ok) => "ok".into(),
        Some(SetupState::Failed(c)) => format!("falhou (código {c})"),
        Some(SetupState::Interrupted) => "interrompido".into(),
    };
    let col_lines = [
        [
            kv("Branch", s.branch.clone().unwrap_or_else(|| "—".into())),
            kv(&ports_label, ports),
            kv("Sessão Claude", format!("{}…", &sid[..8])),
        ],
        [
            kv("Diretório", wt),
            kv("Setup do worktree", setup),
            kv("Modo de permissão", s.permission_mode.clone()),
        ],
        [
            kv("No estado há", since),
            kv("Último evento", event),
            kv(
                "Base do diff",
                s.base.clone().unwrap_or_else(|| "HEAD".into()),
            ),
        ],
    ];
    for (i, group) in col_lines.into_iter().enumerate() {
        let lines: Vec<Line> = group.into_iter().flatten().collect();
        f.render_widget(Paragraph::new(lines), cols[i]);
    }

    let alive = app.alive.contains(&s.tmux_name());
    let pblock = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border()))
        .title(Span::styled(" Preview ao vivo ", Style::new().fg(muted())))
        .title_bottom(
            Line::styled(" Enter entra · Ctrl-q volta ", Style::new().fg(muted())).right_aligned(),
        );
    let pi = pblock.inner(preview);
    f.render_widget(pblock, preview);
    if alive {
        f.render_widget(Paragraph::new(tail(&app.preview, pi.height as usize)), pi);
    } else {
        let resume = if s.started_once {
            "claude --resume"
        } else {
            "claude --session-id"
        };
        f.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    "Sessão suspensa — o processo não está rodando.",
                    Style::new().fg(muted()),
                ),
                Line::raw(""),
                Line::from(vec![
                    Span::raw("Enter retoma com "),
                    Span::styled(
                        format!("{resume} {}", s.claude_session_id),
                        Style::new().fg(status_color(Status::Working)),
                    ),
                ]),
                Line::styled(format!("em {}", s.cwd.display()), Style::new().fg(muted())),
            ]),
            pi,
        );
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

fn rounded(color: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
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
    let msg = app
        .last_message(s)
        .unwrap_or_else(|| "(nenhum turno concluído ainda)".into());
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

fn truncate_left(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 2 {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n - (max - 1)).collect();
    format!("…{tail}")
}

/// Últimas `h` linhas não vazias do capture-pane.
fn tail(text: &Text<'static>, h: usize) -> Text<'static> {
    let mut lines = text.lines.clone();
    while lines
        .last()
        .is_some_and(|l| l.spans.iter().all(|s| s.content.trim().is_empty()))
    {
        lines.pop();
    }
    let start = lines.len().saturating_sub(h);
    Text::from(lines.split_off(start))
}

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
    let sel_status = app.selected_session().map(|s| app.status_of(s));
    let mut keys: Vec<(&'static str, &'static str)> = vec![("Enter", "entrar")];
    match sel_status {
        Some(Status::Approval) => keys.extend([("y", "aprovar"), ("x", "recusar")]),
        Some(Status::Yours | Status::Error) => keys.push(("m", "responder")),
        _ => {}
    }
    if sel_status.is_some() {
        keys.push(("D", "diff"));
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
        Modal::Help => {
            let r = centered(area, 66, 22);
            f.render_widget(Clear, r);
            let rows = [
                ("j/k ↑/↓", "navegar"),
                ("h/l ←/→", "recolher / expandir projeto"),
                ("Enter", "entrar na sessão (retoma se suspensa)"),
                ("Ctrl-q", "dentro da sessão: voltar ao ccdeck"),
                ("y / x", "aprovar / recusar o pedido pendente"),
                ("m", "responder sem entrar na sessão"),
                ("D", "diff completo da sessão"),
                ("n / N", "nova sessão / novo projeto"),
                ("a ou Tab", "alternar visão Atenção"),
                ("/", "filtrar (Esc limpa)"),
                ("e", "editar notas (Ctrl-s salva)"),
                ("r", "renomear sessão"),
                ("d", "remover sessão ou projeto vazio"),
                ("q", "sair — as sessões continuam rodando"),
            ];
            let lines: Vec<Line> = rows
                .iter()
                .map(|(k, v)| {
                    Line::from(vec![
                        Span::styled(format!("{k:<10}"), Style::new().fg(accent()).bold()),
                        Span::styled(*v, Style::new().fg(fg())),
                    ])
                })
                .collect();
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
            let r = centered(area, 78, 15 + prep_h);
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
                    if i == form.mode {
                        format!("(•) {m}")
                    } else {
                        format!("( ) {m}")
                    }
                })
                .collect::<Vec<_>>()
                .join("  ");
            let mut lines = Vec::new();
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
