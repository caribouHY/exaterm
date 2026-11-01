const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;

const OPT_ECHO: u8 = 1;
const OPT_SUPPRESS_GO_AHEAD: u8 = 3;
const OPT_TERMINAL_TYPE: u8 = 24;
const OPT_NAWS: u8 = 31;

const TERMINAL_TYPE_IS: u8 = 0;
const TERMINAL_TYPE_SEND: u8 = 1;

const MAX_SUBNEGOTIATION_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParseState {
    Data,
    Iac,
    Negotiation(u8),
    Subnegotiation,
    SubnegotiationIac,
    DiscardSubnegotiation,
    DiscardSubnegotiationIac,
}

#[derive(Debug, Clone)]
pub(super) struct TelnetParser {
    state: ParseState,
    subnegotiation: Vec<u8>,
    cols: u16,
    rows: u16,
}

impl TelnetParser {
    pub(super) fn new(cols: u32, rows: u32) -> Self {
        Self {
            state: ParseState::Data,
            subnegotiation: Vec::new(),
            cols: clamp_dimension(cols),
            rows: clamp_dimension(rows),
        }
    }

    pub(super) fn parse(&mut self, input: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut data = Vec::new();
        let mut response = Vec::new();

        for &byte in input {
            match self.state.clone() {
                ParseState::Data => {
                    if byte == IAC {
                        self.state = ParseState::Iac;
                    } else {
                        data.push(byte);
                    }
                }
                ParseState::Iac => match byte {
                    IAC => {
                        data.push(IAC);
                        self.state = ParseState::Data;
                    }
                    DO | DONT | WILL | WONT => {
                        self.state = ParseState::Negotiation(byte);
                    }
                    SB => {
                        self.subnegotiation.clear();
                        self.state = ParseState::Subnegotiation;
                    }
                    SE => {
                        self.state = ParseState::Data;
                    }
                    _ => {
                        self.state = ParseState::Data;
                    }
                },
                ParseState::Negotiation(command) => {
                    response.extend(negotiate(command, byte, self.cols, self.rows));
                    self.state = ParseState::Data;
                }
                ParseState::Subnegotiation => {
                    if byte == IAC {
                        self.state = ParseState::SubnegotiationIac;
                    } else {
                        self.push_subnegotiation(byte);
                    }
                }
                ParseState::SubnegotiationIac => {
                    if byte == IAC {
                        self.state = ParseState::Subnegotiation;
                        self.push_subnegotiation(IAC);
                    } else if byte == SE {
                        response.extend(handle_subnegotiation(
                            &self.subnegotiation,
                            self.cols,
                            self.rows,
                        ));
                        self.subnegotiation.clear();
                        self.state = ParseState::Data;
                    } else {
                        self.subnegotiation.clear();
                        self.state = ParseState::DiscardSubnegotiation;
                    }
                }
                ParseState::DiscardSubnegotiation => {
                    if byte == IAC {
                        self.state = ParseState::DiscardSubnegotiationIac;
                    }
                }
                ParseState::DiscardSubnegotiationIac => {
                    self.state = if byte == SE {
                        ParseState::Data
                    } else {
                        ParseState::DiscardSubnegotiation
                    };
                }
            }
        }

        (data, response)
    }

    pub(super) fn resize(&mut self, cols: u32, rows: u32) -> Vec<u8> {
        self.cols = clamp_dimension(cols);
        self.rows = clamp_dimension(rows);
        build_naws(self.cols, self.rows)
    }

    fn push_subnegotiation(&mut self, byte: u8) {
        if self.subnegotiation.len() == MAX_SUBNEGOTIATION_BYTES {
            // A rejected frame stays opaque until its unescaped terminator, so its
            // remaining bytes cannot become terminal output or option commands.
            self.subnegotiation.clear();
            self.state = ParseState::DiscardSubnegotiation;
        } else {
            self.subnegotiation.push(byte);
        }
    }
}

pub(super) fn clamp_dimension(value: u32) -> u16 {
    u16::try_from(value.max(1).min(u16::MAX as u32)).unwrap_or(u16::MAX)
}

fn command_response(command: u8, option: u8) -> Vec<u8> {
    vec![IAC, command, option]
}

fn negotiate(command: u8, option: u8, cols: u16, rows: u16) -> Vec<u8> {
    match command {
        WILL => match option {
            OPT_ECHO | OPT_SUPPRESS_GO_AHEAD => command_response(DO, option),
            _ => command_response(DONT, option),
        },
        DO => match option {
            OPT_SUPPRESS_GO_AHEAD | OPT_TERMINAL_TYPE => command_response(WILL, option),
            OPT_NAWS => {
                let mut response = command_response(WILL, option);
                response.extend(build_naws(cols, rows));
                response
            }
            _ => command_response(WONT, option),
        },
        WONT => command_response(DONT, option),
        DONT => command_response(WONT, option),
        _ => Vec::new(),
    }
}

fn handle_subnegotiation(data: &[u8], cols: u16, rows: u16) -> Vec<u8> {
    if data == [OPT_TERMINAL_TYPE, TERMINAL_TYPE_SEND] {
        build_terminal_type()
    } else if data.first() == Some(&OPT_NAWS) {
        build_naws(cols, rows)
    } else {
        Vec::new()
    }
}

fn build_terminal_type() -> Vec<u8> {
    let mut response = vec![IAC, SB, OPT_TERMINAL_TYPE, TERMINAL_TYPE_IS];
    response.extend(b"xterm-256color");
    response.extend([IAC, SE]);
    response
}

pub(super) fn build_naws(cols: u16, rows: u16) -> Vec<u8> {
    let [cols_hi, cols_lo] = cols.to_be_bytes();
    let [rows_hi, rows_lo] = rows.to_be_bytes();
    let mut response = vec![IAC, SB, OPT_NAWS];
    response.extend(escape_iac_bytes(vec![cols_hi, cols_lo, rows_hi, rows_lo]));
    response.extend([IAC, SE]);
    response
}

pub(super) fn escape_user_data(data: Vec<u8>) -> Vec<u8> {
    escape_iac_bytes(data)
}

fn escape_iac_bytes(data: Vec<u8>) -> Vec<u8> {
    let mut escaped = Vec::with_capacity(data.len());
    for byte in data {
        escaped.push(byte);
        if byte == IAC {
            escaped.push(IAC);
        }
    }
    escaped
}

#[cfg(test)]
#[path = "codec/tests.rs"]
mod tests;
