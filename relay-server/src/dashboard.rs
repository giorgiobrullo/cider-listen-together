//! Terminal dashboard for the relay server

use crate::metrics::{LogLevel, Metrics, ServerStatus};
use crate::network::{self, NetworkEvent};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use parking_lot::RwLock;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame, Terminal,
};
use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Dashboard state for scrolling etc.
struct DashboardState {
    /// Log scroll position (0 = most recent at bottom)
    log_scroll: usize,
    /// Whether auto-scroll is enabled (follows new logs)
    auto_scroll: bool,
}

/// Run the dashboard
pub async fn run(metrics: Arc<RwLock<Metrics>>) -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Channel for network events
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    // Start network in background
    let metrics_for_network = Arc::clone(&metrics);
    tokio::spawn(async move {
        if let Err(e) = network::run_with_dashboard(metrics_for_network, event_tx).await {
            eprintln!("Network error: {}", e);
        }
    });

    // Dashboard state
    let mut state = DashboardState {
        log_scroll: 0,
        auto_scroll: true,
    };

    // Main loop
    let tick_rate = Duration::from_millis(100);
    let mut should_quit = false;

    while !should_quit {
        // Handle network events
        while let Ok(event) = event_rx.try_recv() {
            match event {
                NetworkEvent::Ready { .. } => {}
                NetworkEvent::PublicIp(_) => {}
                NetworkEvent::PortCheck(_) => {}
            }
            // New events came in, scroll to bottom if auto-scroll enabled
            if state.auto_scroll {
                state.log_scroll = 0;
            }
        }

        // Draw
        terminal.draw(|f| draw(f, &metrics, &state))?;

        // Handle input
        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let log_count = metrics.read().logs.len();

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => should_quit = true,
                        KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            should_quit = true
                        }
                        // Scroll up (older logs)
                        KeyCode::Up | KeyCode::Char('k') => {
                            if log_count > 0 {
                                state.log_scroll = (state.log_scroll + 1).min(log_count.saturating_sub(1));
                                state.auto_scroll = false;
                            }
                        }
                        // Scroll down (newer logs)
                        KeyCode::Down | KeyCode::Char('j') => {
                            if state.log_scroll > 0 {
                                state.log_scroll -= 1;
                            }
                            if state.log_scroll == 0 {
                                state.auto_scroll = true;
                            }
                        }
                        // Page up
                        KeyCode::PageUp => {
                            if log_count > 0 {
                                state.log_scroll = (state.log_scroll + 10).min(log_count.saturating_sub(1));
                                state.auto_scroll = false;
                            }
                        }
                        // Page down
                        KeyCode::PageDown => {
                            state.log_scroll = state.log_scroll.saturating_sub(10);
                            if state.log_scroll == 0 {
                                state.auto_scroll = true;
                            }
                        }
                        // Home - oldest logs
                        KeyCode::Home => {
                            if log_count > 0 {
                                state.log_scroll = log_count.saturating_sub(1);
                                state.auto_scroll = false;
                            }
                        }
                        // End - newest logs (enable auto-scroll)
                        KeyCode::End => {
                            state.log_scroll = 0;
                            state.auto_scroll = true;
                        }
                        // Toggle auto-scroll
                        KeyCode::Char('a') => {
                            state.auto_scroll = !state.auto_scroll;
                            if state.auto_scroll {
                                state.log_scroll = 0;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

/// Draw the dashboard
fn draw(f: &mut Frame, metrics: &Arc<RwLock<Metrics>>, state: &DashboardState) {
    let m = metrics.read();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(9),  // Stats
            Constraint::Min(10),    // Logs
            Constraint::Length(1),  // Footer
        ])
        .split(f.area());

    // Header
    draw_header(f, chunks[0], &m);

    // Stats
    draw_stats(f, chunks[1], &m);

    // Logs
    draw_logs(f, chunks[2], &m, state);

    // Footer
    draw_footer(f, chunks[3], state);
}

fn draw_header(f: &mut Frame, area: Rect, m: &Metrics) {
    let status_style = match m.status {
        ServerStatus::Starting => Style::default().fg(Color::Yellow),
        ServerStatus::Running => Style::default().fg(Color::Green),
        ServerStatus::Error => Style::default().fg(Color::Red),
    };

    let status_text = match m.status {
        ServerStatus::Starting => "STARTING",
        ServerStatus::Running => "RUNNING",
        ServerStatus::Error => "ERROR",
    };

    let title = vec![
        Line::from(vec![
            Span::styled("Cider Relay Server", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  │  Status: "),
            Span::styled(status_text, status_style),
            Span::raw("  │  Uptime: "),
            Span::styled(m.uptime(), Style::default().fg(Color::Cyan)),
        ])
    ];

    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL).title(" Dashboard "));

    f.render_widget(header, area);
}

fn draw_stats(f: &mut Frame, area: Rect, m: &Metrics) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    // Server Info
    let peer_id_short = m.peer_id.as_ref()
        .map(|p| if p.len() > 20 { format!("{}...", &p[..20]) } else { p.clone() })
        .unwrap_or_else(|| "...".to_string());

    let ip_display = m.public_ip.as_ref()
        .map(|ip| {
            let reachable = match m.tcp_reachable {
                Some(true) => " ✓",
                Some(false) => " ✗",
                None => " ?",
            };
            format!("{}{}", ip, reachable)
        })
        .unwrap_or_else(|| "detecting...".to_string());

    let server_info = vec![
        Line::from(vec![
            Span::raw("Peer ID: "),
            Span::styled(&peer_id_short, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("Public IP: "),
            Span::styled(&ip_display, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Ports: "),
            Span::styled(format!("TCP:{} QUIC:{}", m.tcp_port, m.quic_port), Style::default().fg(Color::Cyan)),
        ]),
    ];

    let server_block = Paragraph::new(server_info)
        .block(Block::default().borders(Borders::ALL).title(" Server "));
    f.render_widget(server_block, chunks[0]);

    // Connections
    let conn_info = vec![
        Line::from(vec![
            Span::raw("Active: "),
            Span::styled(
                m.connected_peers.to_string(),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Total: "),
            Span::styled(m.total_connections.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("Peak: "),
            Span::styled(m.peak_connections.to_string(), Style::default().fg(Color::Magenta)),
        ]),
    ];

    let conn_block = Paragraph::new(conn_info)
        .block(Block::default().borders(Borders::ALL).title(" Connections "));
    f.render_widget(conn_block, chunks[1]);

    // Relay Stats
    let relay_info = vec![
        Line::from(vec![
            Span::raw("Reservations: "),
            Span::styled(
                format!("{} / {}", m.active_reservations, m.total_reservations),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("Circuits: "),
            Span::styled(
                format!("{} / {}", m.active_circuits, m.total_circuits),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("Relayed: "),
            Span::styled(format_bytes(m.bytes_relayed), Style::default().fg(Color::Green)),
        ]),
    ];

    let relay_block = Paragraph::new(relay_info)
        .block(Block::default().borders(Borders::ALL).title(" Relay "));
    f.render_widget(relay_block, chunks[2]);
}

fn draw_logs(f: &mut Frame, area: Rect, m: &Metrics, state: &DashboardState) {
    let visible_height = area.height.saturating_sub(2) as usize;
    let total_logs = m.logs.len();

    // Calculate which logs to show based on scroll position
    let log_items: Vec<ListItem> = m.logs
        .iter()
        .rev()
        .skip(state.log_scroll)
        .take(visible_height)
        .map(|entry| {
            let level_style = match entry.level {
                LogLevel::Info => Style::default().fg(Color::Blue),
                LogLevel::Warning => Style::default().fg(Color::Yellow),
                LogLevel::Error => Style::default().fg(Color::Red),
                LogLevel::Connection => Style::default().fg(Color::Green),
                LogLevel::Relay => Style::default().fg(Color::Magenta),
            };

            let time = entry.timestamp.format("%H:%M:%S").to_string();

            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", time), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("[{}] ", entry.level.as_str()), level_style),
                Span::raw(&entry.message),
            ]))
        })
        .collect();

    // Title with scroll indicator
    let scroll_indicator = if state.auto_scroll {
        " [AUTO] ".to_string()
    } else if total_logs > 0 {
        format!(" [{}/{}] ", total_logs - state.log_scroll, total_logs)
    } else {
        String::new()
    };

    let logs = List::new(log_items)
        .block(Block::default().borders(Borders::ALL).title(format!(" Activity Log{}", scroll_indicator)));

    f.render_widget(logs, area);

    // Render scrollbar if there are more logs than visible
    if total_logs > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(total_logs)
            .position(total_logs.saturating_sub(state.log_scroll + visible_height));

        f.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }
}

fn draw_footer(f: &mut Frame, area: Rect, state: &DashboardState) {
    let auto_text = if state.auto_scroll { "ON " } else { "OFF" };
    let auto_color = if state.auto_scroll { Color::Green } else { Color::Yellow };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Q ", Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(" Quit  "),
        Span::styled(" ↑↓ ", Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(" Scroll  "),
        Span::styled(" PgUp/Dn ", Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(" Page  "),
        Span::styled(" A ", Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(" Auto-scroll: "),
        Span::styled(auto_text, Style::default().fg(auto_color)),
    ]));

    f.render_widget(footer, area);
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
