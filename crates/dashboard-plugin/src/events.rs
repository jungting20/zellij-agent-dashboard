use zellij_tile::{
    prelude::{BareKey, Event},
    shim::plugin_api::{
        event::{ProtobufEvent, ProtobufEventPayload, ProtobufEventType},
        key::ProtobufMainKey,
    },
};

/// zellij-utils 0.45.0 decodes character keys with `i32 as u8 as char`.
/// Recover the Unicode scalar from the wire before that lossy conversion.
pub fn decode_event(wire: ProtobufEvent) -> Result<Event, &'static str> {
    let character = match (&wire.payload, ProtobufEventType::try_from(wire.name)) {
        (Some(ProtobufEventPayload::KeyPayload(key)), Ok(ProtobufEventType::Key)) => {
            match key.main_key {
                Some(ProtobufMainKey::Char(value)) => Some(
                    u32::try_from(value)
                        .ok()
                        .and_then(char::from_u32)
                        .ok_or("invalid Unicode key")?,
                ),
                _ => None,
            }
        }
        _ => None,
    };
    let mut event: Event = wire.try_into()?;
    if let (Event::Key(key), Some(character)) = (&mut event, character) {
        key.bare_key = BareKey::Char(character);
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zellij_tile::{prelude::KeyWithModifier, shim::prost::Message};

    #[test]
    fn character_keys_survive_the_protobuf_round_trip() {
        for character in "한글 ㄱㅏ 한 日本語 😀 é A\\".chars() {
            let key = KeyWithModifier::new(BareKey::Char(character));
            let wire: ProtobufEvent = Event::Key(key.clone()).try_into().unwrap();
            let bytes = wire.encode_to_vec();
            let decoded = decode_event(ProtobufEvent::decode(bytes.as_slice()).unwrap()).unwrap();
            assert!(matches!(decoded, Event::Key(actual) if actual == key));
        }
    }

    #[test]
    fn modifiers_named_keys_and_other_events_keep_their_sdk_behavior() {
        for key in [
            KeyWithModifier::new(BareKey::Char('한')).with_shift_modifier(),
            KeyWithModifier::new(BareKey::Char('e')).with_ctrl_modifier(),
            KeyWithModifier::new(BareKey::Enter).with_alt_modifier(),
            KeyWithModifier::new(BareKey::Backspace),
            KeyWithModifier::new(BareKey::Left),
        ] {
            let wire = Event::Key(key.clone()).try_into().unwrap();
            assert!(matches!(decode_event(wire).unwrap(), Event::Key(actual) if actual == key));
        }
        let wire = Event::PastedText("한글\n둘째 줄".into())
            .try_into()
            .unwrap();
        assert!(
            matches!(decode_event(wire).unwrap(), Event::PastedText(text) if text == "한글\n둘째 줄")
        );
        let wire = Event::Visible(true).try_into().unwrap();
        assert!(matches!(decode_event(wire).unwrap(), Event::Visible(true)));
    }

    #[test]
    fn session_updates_keep_current_session_and_renames_without_a_sync_query() {
        use zellij_tile::prelude::SessionInfo;
        for name in ["운영", "renamed session"] {
            let sessions = vec![
                SessionInfo {
                    name: "other".into(),
                    ..SessionInfo::default()
                },
                SessionInfo {
                    name: name.into(),
                    is_current_session: true,
                    ..SessionInfo::default()
                },
            ];
            let wire: ProtobufEvent = Event::SessionUpdate(sessions, vec![]).try_into().unwrap();
            let Event::SessionUpdate(decoded, _) = decode_event(wire).unwrap() else {
                panic!("wrong event")
            };
            assert_eq!(
                decoded.iter().find(|s| s.is_current_session).unwrap().name,
                name
            );
        }
    }

    #[test]
    fn invalid_scalars_are_rejected_instead_of_becoming_shortcuts() {
        for value in [-1, 0xd800, 0x110000] {
            let mut wire: ProtobufEvent = Event::Key(KeyWithModifier::new(BareKey::Char('a')))
                .try_into()
                .unwrap();
            if let Some(ProtobufEventPayload::KeyPayload(key)) = &mut wire.payload {
                key.main_key = Some(ProtobufMainKey::Char(value));
            }
            assert!(decode_event(wire).is_err());
        }
    }
}
