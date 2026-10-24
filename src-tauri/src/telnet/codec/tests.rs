use super::*;

#[test]
fn parser_preserves_data_and_negotiation_at_every_receive_boundary() {
    let input = [
        b'a',
        IAC,
        IAC,
        b'b',
        IAC,
        DO,
        OPT_NAWS,
        IAC,
        SB,
        OPT_TERMINAL_TYPE,
        TERMINAL_TYPE_SEND,
        IAC,
        SE,
        b'c',
    ];
    let mut expected_response = vec![IAC, WILL, OPT_NAWS];
    expected_response.extend(build_naws(120, 30));
    expected_response.extend(build_terminal_type());
    for split in 0..=input.len() {
        let mut parser = TelnetParser::new(120, 30);
        let (mut data, mut response) = parser.parse(&input[..split]);
        let (tail_data, tail_response) = parser.parse(&input[split..]);
        data.extend(tail_data);
        response.extend(tail_response);
        assert_eq!(data, [b'a', IAC, b'b', b'c'], "split {split}");
        assert_eq!(response, expected_response, "split {split}");
    }
}

#[test]
fn subnegotiation_limit_accepts_boundary_and_rejects_next_decoded_byte() {
    for length in [
        MAX_SUBNEGOTIATION_BYTES - 1,
        MAX_SUBNEGOTIATION_BYTES,
        MAX_SUBNEGOTIATION_BYTES + 1,
    ] {
        let mut parser = TelnetParser::new(120, 30);
        parser.parse(&[IAC, SB, OPT_NAWS]);
        for _ in 1..length {
            let (data, response) = parser.parse(&[IAC, IAC]);
            assert!(data.is_empty());
            assert!(response.is_empty());
            assert!(parser.subnegotiation.len() <= MAX_SUBNEGOTIATION_BYTES);
        }
        let (data, response) = parser.parse(&[IAC, SE, b'x']);
        assert_eq!(data, b"x");
        if length <= MAX_SUBNEGOTIATION_BYTES {
            assert_eq!(response, build_naws(120, 30));
        } else {
            assert!(response.is_empty());
        }
        assert!(parser.subnegotiation.is_empty());
        assert_eq!(parser.state, ParseState::Data);
    }
}

#[test]
fn unterminated_subnegotiation_stays_bounded_and_escaped_se_does_not_end_discard() {
    let mut parser = TelnetParser::new(120, 30);
    parser.parse(&[IAC, SB, OPT_TERMINAL_TYPE]);
    let chunk = [b'z'; 4096];
    for _ in 0..1024 {
        let (data, response) = parser.parse(&chunk);
        assert!(data.is_empty());
        assert!(response.is_empty());
        assert!(parser.subnegotiation.len() <= MAX_SUBNEGOTIATION_BYTES);
    }
    for byte in [IAC, IAC, SE, b'h', IAC, DO, OPT_NAWS] {
        let (data, response) = parser.parse(&[byte]);
        assert!(data.is_empty());
        assert!(response.is_empty());
    }
    assert_eq!(parser.state, ParseState::DiscardSubnegotiation);
    assert!(parser.subnegotiation.is_empty());
    assert!(parser.subnegotiation.capacity() <= MAX_SUBNEGOTIATION_BYTES);
    assert_eq!(
        parser.parse(&[IAC, SE, b'o', b'k']),
        (b"ok".to_vec(), vec![])
    );
    assert_eq!(
        parser.parse(&[IAC, DO, OPT_NAWS]).1,
        [vec![IAC, WILL, OPT_NAWS], build_naws(120, 30)].concat()
    );
}

#[test]
fn malformed_subnegotiation_discards_until_terminator() {
    let mut parser = TelnetParser::new(120, 30);
    for byte in [
        IAC,
        SB,
        OPT_TERMINAL_TYPE,
        TERMINAL_TYPE_SEND,
        IAC,
        WILL,
        OPT_ECHO,
        b'x',
        IAC,
        IAC,
        SE,
    ] {
        assert_eq!(parser.parse(&[byte]), (vec![], vec![]));
    }
    assert_eq!(parser.parse(&[IAC, SE, b'x']), (vec![b'x'], vec![]));
    assert_eq!(parser.state, ParseState::Data);
    assert!(parser.subnegotiation.is_empty());
}

#[test]
fn resize_updates_pending_and_later_naws_negotiations() {
    let mut parser = TelnetParser::new(120, 30);
    parser.parse(&[IAC, DO]);
    assert_eq!(parser.resize(255, 511), build_naws(255, 511));
    assert_eq!(
        parser.parse(&[OPT_NAWS]).1,
        [vec![IAC, WILL, OPT_NAWS], build_naws(255, 511)].concat()
    );
    assert_eq!(parser.resize(0, u32::MAX), build_naws(1, u16::MAX));
    assert_eq!(
        parser.parse(&[IAC, DO, OPT_NAWS]).1,
        [vec![IAC, WILL, OPT_NAWS], build_naws(1, u16::MAX)].concat()
    );
    assert_eq!(
        parser.parse(&[IAC, SB, OPT_NAWS, IAC, SE]).1,
        build_naws(1, u16::MAX)
    );
}

#[test]
fn parser_passes_plain_data_through() {
    let mut parser = TelnetParser::new(120, 30);
    let (data, response) = parser.parse(b"login: ");

    assert_eq!(data, b"login: ");
    assert!(response.is_empty());
}

#[test]
fn parser_negotiates_supported_and_unsupported_options() {
    let mut parser = TelnetParser::new(120, 30);
    let (_data, response) = parser.parse(&[
        IAC, WILL, OPT_ECHO, IAC, DO, OPT_NAWS, IAC, DO, 99, IAC, WILL, 100,
    ]);

    let mut expected = vec![IAC, DO, OPT_ECHO, IAC, WILL, OPT_NAWS];
    expected.extend(build_naws(120, 30));
    expected.extend([IAC, WONT, 99, IAC, DONT, 100]);
    assert_eq!(response, expected);
}

#[test]
fn parser_responds_to_terminal_type_send() {
    let mut parser = TelnetParser::new(120, 30);
    let (_data, response) =
        parser.parse(&[IAC, SB, OPT_TERMINAL_TYPE, TERMINAL_TYPE_SEND, IAC, SE]);

    assert_eq!(response, build_terminal_type());
}

#[test]
fn builds_naws_payload() {
    assert_eq!(
        build_naws(120, 30),
        vec![IAC, SB, OPT_NAWS, 0, 120, 0, 30, IAC, SE]
    );
}

#[test]
fn user_data_escapes_iac_bytes() {
    assert_eq!(
        escape_user_data(vec![b'a', IAC, b'b']),
        vec![b'a', IAC, IAC, b'b']
    );
}

const NAWS_DIMENSIONS: [(u16, &[u8]); 5] = [
    (254, &[0, 254]),
    (255, &[0, 255, 255]),
    (256, &[1, 0]),
    (511, &[1, 255, 255]),
    (65535, &[255, 255, 255, 255]),
];

fn expected_naws(cols: &[u8], rows: &[u8]) -> Vec<u8> {
    let mut expected = vec![255, 250, 31];
    expected.extend_from_slice(cols);
    expected.extend_from_slice(rows);
    expected.extend([255, 240]);
    expected
}

#[test]
fn builds_naws_with_255_columns() {
    assert_eq!(
        build_naws(255, 24),
        vec![255, 250, 31, 0, 255, 255, 0, 24, 255, 240]
    );
}

#[test]
fn builds_and_negotiates_naws_boundary_dimensions() {
    for (cols, cols_bytes) in NAWS_DIMENSIONS {
        for (rows, rows_bytes) in NAWS_DIMENSIONS {
            let expected = expected_naws(cols_bytes, rows_bytes);
            assert_eq!(build_naws(cols, rows), expected, "{cols}x{rows}");

            let mut parser = TelnetParser::new(cols.into(), rows.into());
            let (data, response) = parser.parse(&[IAC, DO, OPT_NAWS]);
            let mut negotiation = vec![255, 251, 31];
            negotiation.extend(expected);
            assert!(data.is_empty(), "{cols}x{rows}");
            assert_eq!(response, negotiation, "{cols}x{rows}");
        }
    }
}

fn assert_naws_chunks(cols: u16, rows: u16, chunks: &[&[u8]], expected: &[u8]) {
    let mut parser = TelnetParser::new(cols.into(), rows.into());
    let mut responses = Vec::new();
    for chunk in chunks {
        let (data, response) = parser.parse(chunk);
        assert!(data.is_empty(), "{cols}x{rows}: {chunks:?}");
        responses.extend(response);
    }
    assert_eq!(responses, expected, "{cols}x{rows}: {chunks:?}");
    assert_eq!(parser.state, ParseState::Data);
    assert!(parser.subnegotiation.is_empty());
    let (data, response) = parser.parse(b"after NAWS");
    assert_eq!(data, b"after NAWS");
    assert!(response.is_empty());
}

#[test]
fn parser_handles_naws_at_every_receive_boundary() {
    for (cols, cols_bytes) in NAWS_DIMENSIONS {
        for (rows, rows_bytes) in NAWS_DIMENSIONS {
            let frame = build_naws(cols, rows);
            let expected = expected_naws(cols_bytes, rows_bytes);
            for split in 0..=frame.len() {
                assert_naws_chunks(cols, rows, &[&frame[..split], &frame[split..]], &expected);
            }
            let chunks: Vec<&[u8]> = frame.chunks(1).collect();
            assert_naws_chunks(cols, rows, &chunks, &expected);
        }
    }
}
