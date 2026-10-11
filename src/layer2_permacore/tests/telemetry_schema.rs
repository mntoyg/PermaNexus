use permanexus_permacore::actuator::{
    MicroState, MicroStateSnapshot, TelemetryData, TelemetryUnit,
};
use prost::Message;

#[test]
fn wire_preserves_zero_presence_and_absence() {
    let sample = TelemetryData {
        value: Some(0.0),
        unit: TelemetryUnit::Ratio as i32,
        sampled_at_unix_ns: Some(0),
        observed_state: Some(MicroStateSnapshot {
            state: MicroState::Idle as i32,
            revision: Some(0),
            instance_id: vec![1; 16],
        }),
        ..Default::default()
    };
    let decoded = TelemetryData::decode(sample.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded.value, Some(0.0));
    assert_eq!(decoded.sampled_at_unix_ns, Some(0));
    let state = decoded.observed_state.unwrap();
    assert_eq!(state.revision, Some(0));
    assert_eq!(state.instance_id, vec![1; 16]);

    let absent = TelemetryData::decode(&[][..]).unwrap();
    assert_eq!(absent.value, None);
    assert_eq!(absent.sampled_at_unix_ns, None);
    assert_eq!(absent.observed_state, None);
    assert_eq!(MicroStateSnapshot::decode(&[][..]).unwrap().revision, None);
}

#[test]
fn wire_preserves_unknown_enums_for_validation_and_full_revision_width() {
    // Protobuf decoding is not a validator; future handlers must reject these.
    let sample = TelemetryData {
        unit: 999,
        observed_state: Some(MicroStateSnapshot {
            state: 999,
            revision: Some(u64::MAX),
            instance_id: vec![2; 16],
        }),
        ..Default::default()
    };
    let decoded = TelemetryData::decode(sample.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded.unit, 999);
    assert!(TelemetryUnit::try_from(decoded.unit).is_err());
    let state = decoded.observed_state.unwrap();
    assert_eq!(state.state, 999);
    assert!(MicroState::try_from(state.state).is_err());
    assert_eq!(state.revision, Some(u64::MAX));
}
