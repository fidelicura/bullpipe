use color_eyre::Result;
use crossterm::event;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Cell;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Row;
use ratatui::widgets::Table;
use ratatui::widgets::TableState;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

#[derive(Debug, Clone, Deserialize)]
struct Payload {
    id: String,
    role: String,
}

#[derive(Debug, Clone)]
struct Node {
    id: String,
    role: String,
    last_seen: SystemTime,
}

#[derive(Debug)]
struct App {
    state: TableState,
    nodes: Arc<Mutex<HashMap<String, Node>>>,
}

impl App {
    fn new(nodes: Arc<Mutex<HashMap<String, Node>>>) -> Self {
        Self {
            state: TableState::default().with_selected(0),
            nodes,
        }
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => self.next_row(),
                    KeyCode::Up | KeyCode::Char('k') => self.previous_row(),
                    _ => {}
                }
            }
        }
    }

    fn next_row(&mut self) {
        let nodes = self.nodes.lock().expect("should not poison");
        let len = nodes.len();
        if len == 0 {
            return;
        }

        let i = match self.state.selected() {
            Some(i) => {
                if i >= len - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous_row(&mut self) {
        let nodes = self.nodes.lock().expect("should not poison");
        let len = nodes.len();
        if len == 0 {
            return;
        }

        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn draw(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]);
        let rects = vertical.split(frame.area());

        self.render_table(frame, rects[0]);
        self.render_footer(frame, rects[1]);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD);

        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Cyan);

        let header = ["ID", "Role", "Seen"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let nodes = self.nodes.lock().expect("should not poison");
        let mut sorted_nodes: Vec<_> = nodes.values().collect();
        sorted_nodes.sort_by(|a, b| a.id.cmp(&b.id));

        let rows = sorted_nodes.iter().map(|node| {
            let elapsed = node.last_seen.elapsed().unwrap_or(Duration::from_secs(0)).as_secs();
            let last_seen = format!("{}s ago", elapsed);

            let mut role_name = node.role.clone();
            if elapsed > 2 {
                role_name = "dead".to_string();
            }
            let role_color = match role_name.as_str() {
                "leader" => Color::Green,
                "follower" => Color::Blue,
                "candidate" => Color::Yellow,
                "dead" => Color::Red,
                _ => Color::White,
            };

            Row::new(vec![
                Cell::from(node.id.clone()),
                Cell::from(role_name).style(Style::default().fg(role_color)),
                Cell::from(last_seen),
            ])
            .height(1)
        });

        let widths = [
            Constraint::Length(20),
            Constraint::Length(50),
            Constraint::Length(8),
            Constraint::Min(15),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::bordered()
                    .title(" Nodes Monitor ")
                    .border_type(BorderType::Rounded),
            )
            .highlight_style(selected_style)
            .highlight_symbol(">> ");

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let info = Paragraph::new("(↑/k) up | (↓/j) down | (q/Esc) quit")
            .style(Style::default().fg(Color::Gray))
            .centered()
            .block(Block::bordered().border_type(BorderType::Double));
        frame.render_widget(info, area);
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let nodes: Arc<Mutex<HashMap<String, Node>>> = Arc::new(Mutex::new(HashMap::new()));
    let nodes_clone = Arc::clone(&nodes);

    thread::spawn(move || {
        let socket = UdpSocket::bind("0.0.0.0:9999").expect("should not fail to bind to port 9999");
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("should not fail as non-zero timeout is passed");

        let mut buf = [0u8; 1024];

        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, _addr)) => {
                    let data = &buf[..size];
                    if let Ok(heartbeat) = serde_json::from_slice::<Payload>(data) {
                        let mut nodes = nodes_clone.lock().expect("should not poison");
                        nodes.insert(
                            heartbeat.id.clone(),
                            Node {
                                id: heartbeat.id,
                                role: heartbeat.role,
                                last_seen: SystemTime::now(),
                            },
                        );
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    eprintln!("error receiving data: {}", e);
                }
            }
        }
    });

    let terminal = ratatui::init();
    let app_result = App::new(nodes).run(terminal);
    ratatui::restore();
    app_result
}
