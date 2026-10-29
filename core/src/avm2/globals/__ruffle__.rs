use crate::avm2::activation::Activation;
use crate::avm2::api_version::ApiVersion;
use crate::avm2::bytearray::ByteArrayStorage;
use crate::avm2::error::Error;
use crate::avm2::function::FunctionArgs;
use crate::avm2::object::{ArrayObject, ByteArrayObject, TObject as _};
use crate::avm2::parameters::ParametersExt;
use crate::avm2::value::Value;
use crate::avm2::array::ArrayStorage;
use crate::avm2::{Namespace, QName};
use crate::string::{AvmString, WStr};
use crate::stub::Stub;
use crate::avm2::traits::TraitKind;
use std::borrow::Cow;

#[cfg(not(target_family = "wasm"))]
use std::io::Write;
#[cfg(not(target_family = "wasm"))]
use std::sync::{Mutex, OnceLock};

pub fn stub_method<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let class = args.get_string(activation, 0);
    let method = args.get_string(activation, 1);
    let specifics = args.try_get_string(2);

    let class = Cow::Owned(class.to_utf8_lossy().to_string());
    let method = Cow::Owned(method.to_utf8_lossy().to_string());
    let specifics = specifics.map(|s| Cow::Owned(s.to_utf8_lossy().to_string()));

    activation
        .context
        .stub_tracker
        .encounter(&Stub::Avm2Method {
            class,
            method,
            specifics,
        });

    Ok(Value::Undefined)
}

pub fn stub_getter<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let class = args.get_string(activation, 0);
    let property = args.get_string(activation, 1);

    let class = Cow::Owned(class.to_utf8_lossy().to_string());
    let property = Cow::Owned(property.to_utf8_lossy().to_string());

    activation
        .context
        .stub_tracker
        .encounter(&Stub::Avm2Getter { class, property });

    Ok(Value::Undefined)
}

pub fn stub_setter<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let class = args.get_string(activation, 0);
    let property = args.get_string(activation, 1);

    let class = Cow::Owned(class.to_utf8_lossy().to_string());
    let property = Cow::Owned(property.to_utf8_lossy().to_string());

    activation
        .context
        .stub_tracker
        .encounter(&Stub::Avm2Setter { class, property });

    Ok(Value::Undefined)
}

pub fn stub_constructor<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let class = args.get_string(activation, 0);
    let specifics = args.try_get_string(1);

    let class = Cow::Owned(class.to_utf8_lossy().to_string());
    let specifics = specifics.map(|s| Cow::Owned(s.to_utf8_lossy().to_string()));

    activation
        .context
        .stub_tracker
        .encounter(&Stub::Avm2Constructor { class, specifics });

    Ok(Value::Undefined)
}

pub fn log_warn<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let strings = args
        .iter()
        .map(|a| a.coerce_to_string(activation))
        .collect::<Result<Vec<_>, _>>()?;
    let msg = crate::string::join(&strings, &WStr::from_units(b" "));
    let msg = msg.to_utf8_lossy();
    tracing::warn!("{}", &msg);

    Ok(Value::Undefined)
}

#[cfg(not(target_family = "wasm"))]
static DUMP_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

/// Append one hitbox/hurtbox rectangle to `RUFFLE_DUMP_PATH` (CSV).
pub fn dump_box<'gc>(
    _activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let x = args.get_f64(0);
    let y = args.get_f64(1);
    let w = args.get_f64(2);
    let h = args.get_f64(3);
    let world_x = args.get_f64(4);
    let world_y = args.get_f64(5);
    let power_frame = args.get_u32(6);
    let power_id = args.get_u32(7);
    let kind = args.get_u32(8);
    let frame = crate::DUMP_FRAME.load(std::sync::atomic::Ordering::Relaxed);

    #[cfg(not(target_family = "wasm"))]
    {
        let path = std::env::var("RUFFLE_DUMP_PATH")
            .unwrap_or_else(|_| "boxes.csv".to_string());
        let file = DUMP_FILE.get_or_init(|| {
            Mutex::new(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .expect("open RUFFLE_DUMP_PATH"),
            )
        });
        if let Ok(mut file) = file.lock() {
            let _ = writeln!(
                file,
                "{frame},{kind},{power_id},{power_frame},{x:.3},{y:.3},{w:.3},{h:.3},{world_x:.3},{world_y:.3}"
            );
            let _ = file.flush();
        }
    }

    #[cfg(target_family = "wasm")]
    {
        crate::push_box_line(format!(
            "{frame},{kind},{power_id},{power_frame},{x:.3},{y:.3},{w:.3},{h:.3},{world_x:.3},{world_y:.3}"
        ));
    }

    Ok(Value::Undefined)
}

#[cfg(not(target_family = "wasm"))]
static STATE_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
#[cfg(not(target_family = "wasm"))]
static DEBUG_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
#[cfg(not(target_family = "wasm"))]
static DUMP_STATE_FRAME: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(not(target_family = "wasm"))]
static STATE_TIME_STARTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(not(target_family = "wasm"))]
static STATE_TIME_SEEN_POSITIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(not(target_family = "wasm"))]
static STATE_FIXED_BASE_MS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

/// Deterministic `getTimer()` value for the fixed-time capture.
///
/// The game derives its animation `TIMESTEP` from the real `getTimer()` delta,
/// so two native runs can land on different `Seq` frame cursors. Once frame 0
/// is anchored, advance a virtual clock by exactly `RUFFLE_STATE_TICK_MS` per
/// dumped state frame so animation playback is part of the oracle.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn fixed_state_time_ms() -> Option<u32> {
    use std::sync::atomic::Ordering;
    if !STATE_TIME_STARTED.load(Ordering::Relaxed) {
        return None;
    }
    let fixed = std::env::var("RUFFLE_STATE_FIXED_TIME")
        .map(|value| value != "0")
        .unwrap_or(false);
    if !fixed {
        return None;
    }
    let tick_ms = std::env::var("RUFFLE_STATE_TICK_MS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(16);
    let frame = DUMP_STATE_FRAME.load(Ordering::Relaxed) as u32;
    Some(
        STATE_FIXED_BASE_MS
            .load(Ordering::Relaxed)
            .saturating_add(frame.saturating_mul(tick_ms)),
    )
}

/// Optional development sidecar for the state dump.
///
/// Raw-only `states.jsonl` stays byte-compatible with the oracle; this writes
/// extra physics/input/transform fields to `RUFFLE_DEBUG_PATH`.
#[cfg(not(target_family = "wasm"))]
fn dump_debug_11000<'gc>(
    activation: &mut Activation<'_, 'gc>,
    game: Value<'gc>,
    entities: Value<'gc>,
    frame: u64,
    time: u32,
) {
    let Ok(path) = std::env::var("RUFFLE_DEBUG_PATH") else {
        return;
    };
    let file = DEBUG_FILE.get_or_init(|| {
        Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .expect("open RUFFLE_DEBUG_PATH"),
        )
    });

    let mut players = Vec::new();
    if let Some(object) = entities.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let values: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for value in values {
                let Some(entity_obj) = value.as_object() else {
                    continue;
                };
                let entity = Value::from(entity_obj);
                let input = state_prop(entity, "_-R5q", activation);
                let combat = state_prop(entity, "_-Kc", activation);
                let surface = state_prop(entity, "_-96f", activation);
                let collision = state_prop(entity, "_-O6z", activation);
                let collision_json = if collision.as_object().is_some() {
                    serde_json::json!({
                        "x1": state_num(state_prop(collision, "startX", activation), activation),
                        "y1": state_num(state_prop(collision, "startY", activation), activation),
                        "x2": state_num(state_prop(collision, "_-f1i", activation), activation),
                        "y2": state_num(state_prop(collision, "_-P2s", activation), activation),
                        "type": state_num(state_prop(collision, "type", activation), activation),
                    })
                } else {
                    serde_json::Value::Null
                };
                let surface_json = if surface.as_object().is_some() {
                    serde_json::json!({
                        "x1": state_num(state_prop(surface, "startX", activation), activation),
                        "y1": state_num(state_prop(surface, "startY", activation), activation),
                        "x2": state_num(state_prop(surface, "_-f1i", activation), activation),
                        "y2": state_num(state_prop(surface, "_-P2s", activation), activation),
                        "type": state_num(state_prop(surface, "type", activation), activation),
                    })
                } else {
                    serde_json::Value::Null
                };
                players.push(serde_json::json!({
                    "id": state_num(state_prop(entity, "_-11", activation), activation),
                    "entState": state_num(state_prop(entity, "_-65w", activation), activation),
                    "posX": state_num(call_state(entity, "_-p3V", activation), activation),
                    "posY": state_num(call_state(entity, "_-Sb", activation), activation),
                    "savedX": state_num(call_state(entity, "_-Q5y", activation), activation),
                    "savedY": state_num(call_state(entity, "_-J5a", activation), activation),
                    "appearX": state_num(call_state(entity, "_-52K", activation), activation),
                    "appearY": state_num(call_state(entity, "_-35u", activation), activation),
                    "centerX": state_num(call_state(entity, "_-p5a", activation), activation),
                    "centerY": state_num(call_state(entity, "_-d5F", activation), activation),
                    "velX": state_num(call_state(entity, "_-a3w", activation), activation),
                    "velY": state_num(call_state(entity, "_-M6G", activation), activation),
                    "flashX": state_num(state_prop(entity, "x", activation), activation),
                    "flashY": state_num(state_prop(entity, "y", activation), activation),
                    "facingLeft": state_bool(call_state(entity, "_-r35", activation)),
                    "currState": state_num(state_prop(input, "_-66U", activation), activation),
                    "sentState": state_num(state_prop(input, "_-B5", activation), activation),
                    "downPress": state_num(state_prop(input, "_-76H", activation), activation),
                    "gravity": state_num(state_prop(entity, "_-w3F", activation), activation),
                    "friction": state_num(state_prop(entity, "_-T2P", activation), activation),
                    "airFriction": state_num(state_prop(entity, "_-h5M", activation), activation),
                    "acceleration": state_num(state_prop(entity, "_-m3Q", activation), activation),
                    "airAcceleration": state_num(state_prop(entity, "_-i3E", activation), activation),
                    "runSpeed": state_num(state_prop(entity, "_-M1U", activation), activation),
                    "airRunSpeed": state_num(state_prop(entity, "_-z2v", activation), activation),
                    "elasticity": state_num(state_prop(entity, "_-7c", activation), activation),
                    "aerialJumps": state_num(state_prop(entity, "_-26T", activation), activation),
                    "dashRunning": state_bool(state_prop(entity, "_-w47", activation)),
                    "speedChange": state_num(state_prop(combat, "_-04K", activation), activation),
                    "airSpeedChange": state_num(state_prop(combat, "_-u5j", activation), activation),
                    "accelChange": state_num(state_prop(combat, "_-E16", activation), activation),
                    "airAccelChange": state_num(state_prop(combat, "_-R4I", activation), activation),
                    "hasPower": state_prop(combat, "_-o3I", activation).as_object().is_some(),
                    "surface": surface_json,
                    "collision": collision_json,
                }));
            }
        }
    }

    let debug = serde_json::json!({
        "frame": frame,
        "t": time,
        "serverTime": state_num(state_prop(game, "_-94N", activation), activation),
        "gameState": state_num(state_prop(game, "_-w35", activation), activation),
        "level": node_level(state_prop(game, "_-I1f", activation), activation),
        "levelLayer": node_transform(state_prop(game, "_-N1L", activation), activation),
        "playerEntLayer3D": node_transform(state_prop(game, "_-K15", activation), activation),
        "camera": node_transform(state_prop(game, "_-32C", activation), activation),
        "platforms": moving_platforms(state_prop(game, "_-22K", activation), activation),
        "collisionWorld": collision_world(game, activation),
        "volumes": volumes(state_prop(game, "_-I1f", activation), activation),
        "players": players,
    });
    if let Ok(mut file) = file.lock() {
        let _ = writeln!(file, "{debug}");
        let _ = file.flush();
    }
}

#[cfg(not(target_family = "wasm"))]
fn node_transform<'gc>(
    value: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> serde_json::Value {
    serde_json::json!({
        "x": state_num(state_prop(value, "x", activation), activation),
        "y": state_num(state_prop(value, "y", activation), activation),
        "scaleX": state_num(state_prop(value, "scaleX", activation), activation),
        "scaleY": state_num(state_prop(value, "scaleY", activation), activation),
    })
}

#[cfg(not(target_family = "wasm"))]
fn node_level<'gc>(
    level: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> serde_json::Value {
    let bounds = state_prop(level, "_-f4F", activation);
    serde_json::json!({
        "scale": state_num(state_prop(level, "_-X4E", activation), activation),
        "bounds": {
            "x": state_num(state_prop(bounds, "x", activation), activation),
            "y": state_num(state_prop(bounds, "y", activation), activation),
            "width": state_num(state_prop(bounds, "width", activation), activation),
            "height": state_num(state_prop(bounds, "height", activation), activation),
        },
    })
}

#[cfg(not(target_family = "wasm"))]
fn moving_platforms<'gc>(
    scene: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> serde_json::Value {
    let list = state_prop(scene, "_-c5B", activation);
    let mut platforms = Vec::new();
    if let Some(object) = list.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let values: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for value in values {
                let Some(platform_obj) = value.as_object() else {
                    continue;
                };
                let platform = Value::from(platform_obj);
                let collisions = state_prop(platform, "_-Z5D", activation);
                let collision_count = collisions
                    .as_object()
                    .and_then(|object| object.as_vector_storage().map(|vector| vector.iter().count()))
                    .unwrap_or(0);
                platforms.push(serde_json::json!({
                    "id": state_str(state_prop(platform, "_-94r", activation), activation),
                    "x": state_num(state_prop(platform, "_-G6H", activation), activation),
                    "y": state_num(state_prop(platform, "_-O6b", activation), activation),
                    "disabled": state_bool(state_prop(platform, "_-H2e", activation)),
                    "collisions": collision_count,
                }));
            }
        }
    }
    serde_json::Value::Array(platforms)
}

#[cfg(not(target_family = "wasm"))]
fn collision_lines<'gc>(
    list: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> serde_json::Value {
    let mut lines = Vec::new();
    if let Some(object) = list.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let values: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for value in values {
                let Some(line_obj) = value.as_object() else {
                    continue;
                };
                let line = Value::from(line_obj);
                lines.push(serde_json::json!({
                    "x1": state_num(state_prop(line, "startX", activation), activation),
                    "y1": state_num(state_prop(line, "startY", activation), activation),
                    "x2": state_num(state_prop(line, "_-f1i", activation), activation),
                    "y2": state_num(state_prop(line, "_-P2s", activation), activation),
                    "type": state_num(state_prop(line, "type", activation), activation),
                }));
            }
        }
    }
    serde_json::Value::Array(lines)
}

#[cfg(not(target_family = "wasm"))]
fn collision_world<'gc>(
    game: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> serde_json::Value {
    let manager = state_prop(game, "_-b1x", activation);
    serde_json::json!({
        "static": collision_lines(state_prop(manager, "_-w7", activation), activation),
        "dynamic": collision_lines(state_prop(manager, "_-82D", activation), activation),
    })
}

#[cfg(not(target_family = "wasm"))]
fn volumes<'gc>(
    level: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> serde_json::Value {
    let list = state_prop(level, "_-I49", activation);
    let mut volumes = Vec::new();
    if let Some(object) = list.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let values: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for value in values {
                let Some(volume_obj) = value.as_object() else {
                    continue;
                };
                let volume = Value::from(volume_obj);
                volumes.push(serde_json::json!({
                    "type": state_num(state_prop(volume, "type", activation), activation),
                    "tlX": state_num(state_prop(volume, "_-W3g", activation), activation),
                    "tlY": state_num(state_prop(volume, "_-o55", activation), activation),
                    "brX": state_num(state_prop(volume, "_-Q1v", activation), activation),
                    "brY": state_num(state_prop(volume, "_-q3t", activation), activation),
                }));
            }
        }
    }
    serde_json::Value::Array(volumes)
}

fn state_prop<'gc>(
    obj: Value<'gc>,
    name: &str,
    activation: &mut Activation<'_, 'gc>,
) -> Value<'gc> {
    obj.get_public_property(AvmString::new_utf8(activation.gc(), name), activation)
        .unwrap_or(Value::Undefined)
}

fn state_num<'gc>(value: Value<'gc>, activation: &mut Activation<'_, 'gc>) -> f64 {
    value
        .coerce_to_number(activation)
        .ok()
        .filter(|n| n.is_finite())
        .unwrap_or(0.0)
}

fn state_bool(value: Value<'_>) -> bool {
    matches!(value, Value::Bool(true))
}

fn state_str<'gc>(
    value: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> String {
    value
        .coerce_to_string(activation)
        .map(|s| s.to_utf8_lossy().to_string())
        .unwrap_or_default()
}

fn call_state<'gc>(
    obj: Value<'gc>,
    name: &str,
    activation: &mut Activation<'_, 'gc>,
) -> Value<'gc> {
    obj.call_public_property(
        AvmString::new_utf8(activation.gc(), name),
        FunctionArgs::from_slice(&[]),
        activation,
    )
    .unwrap_or(Value::Undefined)
}

static AUTO_TRAINING_STARTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Set by `load_replay` once the 11.00 token set is detected. The 11.00 entity
/// and game fields use different live names than 10.10, so `dump_state` needs
/// the matching table.
static BUILD_11000: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// True after the diagnostic replay loader has handed a replay to the game.
/// The Main tick calls `dump_state` every frame; before this flips, there is no
/// replay state to read and the token tables do not match the menu entities.
static REPLAY_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static REPLAY_LOAD_STARTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Construct a `BitStream` around raw replay bytes and call
/// `Game._-E6a(name, bitStream)`, the game's replay loader. This bypasses the
/// Replays UI so replay playback can be exercised independently of screen
/// navigation.
fn load_replay<'gc>(
    activation: &mut Activation<'_, 'gc>,
    game: Value<'gc>,
    filename: String,
    bytes: Vec<u8>,
) -> Result<(), Error<'gc>> {
    tracing::warn!("replay load: {} bytes for {filename}", bytes.len());
    REPLAY_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);

    let storage = ByteArrayStorage::from_vec(activation.context, bytes);
    let bytearray = ByteArrayObject::from_storage(activation.context, storage);
    let bytearray_value: Value<'gc> = bytearray.into();

    let package = AvmString::new_utf8(activation.gc(), "");
    let namespace = Namespace::package(package, ApiVersion::VM_INTERNAL, activation.strings());
    let domain = game
        .as_object()
        .and_then(|object| object.instance_class().translation_unit())
        .map(|unit| unit.domain())
        .unwrap_or_else(|| activation.domain());
    tracing::warn!("replay load: resolving BitStream class");
    let mut class_object = None;
    // 11.00 first, then the 10.10 diagnostic build. The class name tells us which
    // token set to use for the rest of the replay path.
    let mut is_11000 = false;
    for name in ["_-Lg", "BitStream", "_-L5S"] {
        let class_name = AvmString::new_utf8(activation.gc(), name);
        let qname = QName::new(namespace, class_name);
        if let Ok(class_value) = domain.get_defined_value(activation, qname) {
            if let Some(found) = class_value
                .as_object()
                .and_then(|object| object.as_class_object())
            {
                class_object = Some(found);
                is_11000 = name == "_-Lg";
                break;
            }
        }
    }
    let Some(class_object) = class_object else {
        return Err("BitStream class was not found".into());
    };
    BUILD_11000.store(is_11000, std::sync::atomic::Ordering::Relaxed);
    tracing::warn!(
        "replay load: build={}",
        if is_11000 { "11.00" } else { "10.10" }
    );

    let name = AvmString::new_utf8(activation.gc(), filename.as_str());

    // Replay files are zlib-compressed + XOR-masked. Decode the ByteArray in
    // place before BitStream/ReplayData see it.
    let ui_class_name = if is_11000 { "_-4p" } else { "_-X2P" };
    let ui_qname = QName::new(
        namespace,
        AvmString::new_utf8(activation.gc(), ui_class_name),
    );
    if let Ok(ui_class_value) = domain.get_defined_value(activation, ui_qname) {
        let sub_screen_prop = if is_11000 { "_-E4w" } else { "_-n4T" };
        if let Ok(sub_screen) = ui_class_value.get_public_property(
            AvmString::new_utf8(activation.gc(), sub_screen_prop),
            activation,
        ) {
            if sub_screen.as_object().is_some() {
                let decode_method = if is_11000 { "_-B1a" } else { "_-i23" };
                tracing::warn!(
                    "replay load: decoding replay bytes with {ui_class_name}.{sub_screen_prop}.{decode_method}"
                );
                sub_screen
                    .call_public_property(
                        AvmString::new_utf8(activation.gc(), decode_method),
                        FunctionArgs::from_slice(&[bytearray_value, name.into()]),
                        activation,
                    )
                    .unwrap_or(Value::Undefined);
            }
        }
    }

    tracing::warn!("replay load: constructing BitStream");
    let bitstream = class_object.construct(activation, &[bytearray_value, 0.into()])?;

    // Parse the replay in a throwaway ReplayData first. This reports the exact
    // reason LoadFromBitStream rejects a file before the game loader swallows it.
    let replay_class_name = if is_11000 { "_-24q" } else { "_-P27" };
    let replay_qname = QName::new(
        namespace,
        AvmString::new_utf8(activation.gc(), replay_class_name),
    );
    // The diagnostic parse consumes the BitStream. 11.00's BitStream does not
    // reliably reset the backing ByteArray, so only run it on the 10.10 build.
    if !is_11000 {
        if let Ok(replay_class_value) = domain.get_defined_value(activation, replay_qname) {
        if let Some(replay_class) = replay_class_value
            .as_object()
            .and_then(|object| object.as_class_object())
        {
            let replay_data = replay_class.construct(activation, &[])?;
            if replay_data.as_object().is_some() {
                let replay_obj = replay_data;
                let load_method = if is_11000 { "_-b2L" } else { "_-M3t" };
                let parsed = replay_data
                    .call_public_property(
                        AvmString::new_utf8(activation.gc(), load_method),
                        FunctionArgs::from_slice(&[name.into(), bitstream]),
                        activation,
                    )
                    .unwrap_or(Value::Undefined);
                let mut prop = |key: &str| {
                    replay_obj
                        .get_public_property(AvmString::new_utf8(activation.gc(), key), activation)
                        .unwrap_or(Value::Undefined)
                };
                let (success_key, wrong_key, level_key, players_key, names_key, ids_key) = if is_11000 {
                    ("_-Y2j", "_-n4v", "_-T6w", "_-O19", "_-F2S", "_-a4V")
                } else {
                    ("_-s2g", "_-A5S", "_-Y3v", "_-M5T", "_-A4A", "_-I6F")
                };
                let success = matches!(prop(success_key), Value::Bool(true));
                let wrong_version = matches!(prop(wrong_key), Value::Bool(true));
                let level = prop(level_key);
                let array_len = |value: Value<'gc>| {
                    if let Some(object) = value.as_object() {
                        object
                            .as_array_storage()
                            .map(|array| array.length())
                            .unwrap_or(0)
                    } else {
                        0
                    }
                };
                let vector_len = |value: Value<'gc>| {
                    value
                        .as_object()
                        .and_then(|object| object.as_vector_storage().map(|vector| vector.iter().count()))
                        .unwrap_or(0)
                };
                let _players = array_len(prop(players_key));
                let names = array_len(prop(names_key));
                let ent_ids = vector_len(prop(ids_key));
                let indices = vector_len(prop(if is_11000 { "_-hw" } else { "_-I6F" }));
                let data = array_len(prop(if is_11000 { "_-j1p" } else { "_-M5T" }));
                let total = state_num(prop(players_key), activation);
                tracing::warn!(
                    "replay parse: result={parsed:?} success={success} wrongVersion={wrong_version} level={} total={total} data={data} indices={indices} names={names} entIds={ent_ids}",
                    level.as_object().is_some()
                );
            }
        }
        }
    }

    let game_load = if is_11000 { "_-C21" } else { "_-E6a" };
    tracing::warn!("replay load: calling Game.{game_load}({filename})");
    // The diagnostic parse consumed the first BitStream; hand the game a fresh
    // reader over the same decoded ByteArray.
    let game_bitstream = class_object.construct(activation, &[bytearray_value, 0.into()])?;
    game.call_public_property(
        AvmString::new_utf8(activation.gc(), game_load),
        FunctionArgs::from_slice(&[name.into(), game_bitstream]),
        activation,
    )?;
    tracing::warn!("replay load: Game.{game_load} returned");
    if is_11000 {
        for method in ["_-z1L", "_-Q4W"] {
            let _ = game.call_public_property(
                AvmString::new_utf8(activation.gc(), method),
                FunctionArgs::from_slice(&[]),
                activation,
            );
        }
    }
    let mreplay = if is_11000 { "_-331" } else { "_-J4i" };
    let replay_data = game
        .get_public_property(AvmString::new_utf8(activation.gc(), mreplay), activation)
        .unwrap_or(Value::Undefined);
    tracing::warn!(
        "replay load: mReplayData is {}",
        if replay_data.as_object().is_some() {
            "set"
        } else {
            "null"
        }
    );
    if replay_data.as_object().is_some() {
        if is_11000
            && std::env::var("RUFFLE_REPLAY_AUTOSTART")
                .map(|value| value != "0")
                .unwrap_or(false)
        {
            // The headless SWF stubs the replay-control screen, so nothing else
            // calls BeginReplay. Start playback here with the replay's own seed.
            let seed = replay_data
                .get_public_property(
                    AvmString::new_utf8(activation.gc(), "_-669"),
                    activation,
                )
                .unwrap_or(Value::Undefined);
            tracing::warn!("replay load: calling Game.BeginReplay");
            match game.call_public_property(
                AvmString::new_utf8(activation.gc(), "_-T12"),
                FunctionArgs::from_slice(&[replay_data, seed]),
                activation,
            ) {
                Ok(_) => tracing::warn!("replay load: Game.BeginReplay returned"),
                Err(error) => tracing::warn!("replay load: Game.BeginReplay failed: {error}"),
            }
        }
        let mut read = |key: &str| {
            replay_data
                .get_public_property(AvmString::new_utf8(activation.gc(), key), activation)
                .unwrap_or(Value::Undefined)
        };
        let vector_len = |value: Value<'gc>| {
            value
                .as_object()
                .and_then(|object| object.as_vector_storage().map(|vector| vector.iter().count()))
                .unwrap_or(0)
        };
        let array_len = |value: Value<'gc>| {
            value
                .as_object()
                .and_then(|object| object.as_array_storage().map(|array| array.length()))
                .unwrap_or(0)
        };
        let indices = if is_11000 {
            vector_len(read("_-hw"))
        } else {
            0
        };
        let data = if is_11000 {
            array_len(read("_-j1p"))
        } else {
            0
        };
        let total = if is_11000 {
            state_num(read("_-O19"), activation)
        } else {
            0.0
        };
        tracing::warn!(
            "replay load: mReplayData total={total} data={data} indices={indices}"
        );
    }
    DUMP_STATE_FRAME.store(0, std::sync::atomic::Ordering::Relaxed);
    crate::reset_dump_frame();
    crate::reset_fixed_random();
    STATE_TIME_STARTED.store(false, std::sync::atomic::Ordering::Relaxed);
    STATE_TIME_SEEN_POSITIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    REPLAY_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    let entity_list_key = if is_11000 { "_-5r" } else { "_-Wy" };
    if let Ok(entity_list) = game.get_public_property(
        AvmString::new_utf8(activation.gc(), entity_list_key),
        activation,
    ) {
        if let Some(object) = entity_list.as_object() {
            if let Some(vector) = object.as_vector_storage() {
                tracing::warn!("replay load: game entity list {}", vector.iter().count());
            }
        }
    }
    let entities = find_entities(game, activation);
    if let Some(object) = entities.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            tracing::warn!("replay load: entity count {}", vector.iter().count());
        }
    }
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
fn load_replay_from_env<'gc>(
    activation: &mut Activation<'_, 'gc>,
    game: Value<'gc>,
) -> Result<(), Error<'gc>> {
    let path = std::env::var("RUFFLE_REPLAY_PATH").unwrap_or_default();
    if path.is_empty() {
        return Ok(());
    }
    tracing::warn!("replay load: reading {path}");
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("RUFFLE_REPLAY_PATH read failed: {error}"))?;
    let filename = std::path::Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("replay.replay")
        .to_string();
    load_replay(activation, game, filename, bytes)
}

/// One-shot: start training mode once the Game object exists and the client
/// has finished loading its menu data.
///
/// If `RUFFLE_REPLAY_PATH` is set, load that replay instead of starting
/// training mode.
/// Wait for the managers `Game.LoadReplay`/`BeginReplay` dereference.
/// 11.00 and 10.10 use different live names; the first field tells us which.
fn replay_ready<'gc>(game: Value<'gc>, activation: &mut Activation<'_, 'gc>) -> bool {
    if state_prop(game, "_-3z", activation).as_object().is_some() {
        return state_prop(game, "_-22K", activation).as_object().is_some()
            && state_prop(game, "_-s3D", activation).as_object().is_some()
            && state_prop(game, "_-I1f", activation).as_object().is_some();
    }
    state_prop(game, "_-L2b", activation).as_object().is_some()
        && state_prop(game, "_-k58", activation).as_object().is_some()
        && state_prop(game, "_-J3w", activation).as_object().is_some()
        && state_prop(game, "_-o3i", activation).as_object().is_some()
}

pub fn auto_start_training<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let game = args.get_value(0);
    let time = args.get_u32(1);
    #[cfg(target_family = "wasm")]
    let has_replay = crate::has_replay_bytes();
    #[cfg(not(target_family = "wasm"))]
    let has_replay = std::env::var_os("RUFFLE_REPLAY_PATH").is_some();
    #[cfg(target_family = "wasm")]
    let replay_delay = 8_000;
    #[cfg(not(target_family = "wasm"))]
    let replay_delay = std::env::var("RUFFLE_REPLAY_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_else(|| {
            if std::env::var_os("RUFFLE_REPLAY_ONLY").is_some() {
                8_000
            } else {
                15_000
            }
        });
    let replay_due = has_replay && time > replay_delay;
    if (time > 30_000 || replay_due) && game.as_object().is_some() {
        #[cfg(target_family = "wasm")]
        if crate::has_replay_bytes() {
            // The game creates these managers while the menu finishes loading.
            // Calling the replay loader any earlier dereferences null.
            if !replay_ready(game, activation) {
                return Ok(Value::Undefined);
            }
            if let Some((filename, bytes)) = crate::take_replay_bytes() {
                if !REPLAY_LOAD_STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    if let Err(error) = load_replay(activation, game, filename, bytes) {
                        tracing::warn!("replay load failed: {error}");
                    }
                }
                return Ok(Value::Undefined);
            }
        }

        #[cfg(not(target_family = "wasm"))]
        if std::env::var_os("RUFFLE_REPLAY_PATH").is_some() {
            if !replay_ready(game, activation) {
                return Ok(Value::Undefined);
            }
            if !REPLAY_LOAD_STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                if let Err(error) = load_replay_from_env(activation, game) {
                    tracing::warn!("replay load failed: {error}");
                }
            }
            return Ok(Value::Undefined);
        }

        if !AUTO_TRAINING_STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            let _ = game.call_public_property(
                AvmString::new_utf8(activation.gc(), "_-q1J"),
                FunctionArgs::from_slice(&[]),
                activation,
            );
        }
    }
    Ok(Value::Undefined)
}

static DEBUG_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// One-shot: create the hitbox/hurtbox debug display clips.
pub fn enable_debug<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let game = args.get_value(0);
    let time = args.get_u32(1);
    if time > 15_000
        && game.as_object().is_some()
        && !DEBUG_ENABLED.swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        for method in ["_-Us", "_-s3x"] {
            let _ = game.call_public_property(
                AvmString::new_utf8(activation.gc(), method),
                FunctionArgs::from_slice(&[]),
                activation,
            );
        }
    }
    Ok(Value::Undefined)
}

/// Find the Game's `Vector.<Entity>` by walking its instance slots. This
/// avoids relying on obfuscated field names.
fn find_entities<'gc>(
    game: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> Value<'gc> {
    let Some(game_obj) = game.as_object() else {
        return Value::Undefined;
    };
    let mut class = Some(game_obj.instance_class());
    while let Some(current) = class {
        for t in current.traits() {
            let TraitKind::Slot { slot_id, .. } = t.kind() else {
                continue;
            };
            let value = game_obj.get_slot(*slot_id);
            let Some(value_obj) = value.as_object() else {
                continue;
            };
            let Some(vector) = value_obj.as_vector_storage() else {
                continue;
            };
            let first = vector.iter().next();
            drop(vector);
            let Some(first) = first else {
                continue;
            };
            let Some(first_obj) = first.as_object() else {
                continue;
            };
            let id_key = if BUILD_11000.load(std::sync::atomic::Ordering::Relaxed) {
                "_-11"
            } else {
                "_-H4y"
            };
            if !matches!(
                state_prop(Value::from(first_obj), id_key, activation),
                Value::Undefined | Value::Null
            ) {
                return value;
            }
        }
        class = current.super_class();
    }
    Value::Undefined
}

/// Dump the live world items (spawned weapons, gadgets, crates and thrown
/// weapons). `WorldItem` is not an `Entity`, so it lives in `ItemManager`.
fn dump_items<'gc>(
    game: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> Vec<serde_json::Value> {
    let mut items = Vec::new();
    let manager = state_prop(game, "_-82n", activation);
    let world_items = state_prop(manager, "_-S1h", activation);
    let Some(object) = world_items.as_object() else {
        return items;
    };
    let Some(vector) = object.as_vector_storage() else {
        return items;
    };
    let values: Vec<Value<'gc>> = vector.iter().collect();
    drop(vector);

    for value in values {
        let Some(item_obj) = value.as_object() else {
            continue;
        };
        let item = Value::from(item_obj);
        let data = state_prop(item, "_-h4g", activation);
        let item_type = state_prop(item, "_-13A", activation);
        let spawn_bot = state_prop(item, "_-o1G", activation);

        let mut spawn_bot_json = serde_json::Value::Null;
        if let Some(spawn_bot_obj) = spawn_bot.as_object() {
            let spawn_bot = Value::from(spawn_bot_obj);
            let parent = state_prop(spawn_bot, "_-wV", activation);
            let held = state_prop(spawn_bot, "_-z5", activation);
            spawn_bot_json = serde_json::json!({
                "x": state_num(state_prop(spawn_bot, "_-a34", activation), activation),
                "y": state_num(state_prop(spawn_bot, "_-J5u", activation), activation),
                "state": state_num(state_prop(spawn_bot, "_-c5l", activation), activation),
                "parentId": state_num(state_prop(parent, "_-H4y", activation), activation),
                "heldItemId": state_num(
                    state_prop(state_prop(held, "_-h4g", activation), "_-d4w", activation),
                    activation,
                ),
            });
        }

        items.push(serde_json::json!({
            "id": state_num(state_prop(data, "_-d4w", activation), activation),
            "name": state_str(state_prop(item_type, "_-w59", activation), activation),
            "x": state_num(call_state(item, "_-96p", activation), activation),
            "y": state_num(call_state(item, "_-W2z", activation), activation),
            "vx": state_num(state_prop(item, "_-k3r", activation), activation),
            "vy": state_num(state_prop(item, "_-i3F", activation), activation),
            "state": state_num(state_prop(item, "_-d57", activation), activation),
            "team": state_num(state_prop(item, "_-L4W", activation), activation),
            "thrownBy": state_num(state_prop(item, "_-V44", activation), activation),
            "thrownByTeam": state_num(state_prop(item, "_-Ri", activation), activation),
            "thrownTime": state_num(state_prop(item, "_-k9", activation), activation),
            "spawnTick": state_num(state_prop(item, "_-150", activation), activation),
            "stuck": state_bool(state_prop(item, "_-25f", activation)),
            "tossed": state_bool(state_prop(item, "_-O18", activation)),
            "createdAt": state_num(state_prop(data, "_-Y4k", activation), activation),
            "owner": state_num(state_prop(data, "_-M2V", activation), activation),
            "lastHolder": state_num(state_prop(data, "_-u1u", activation), activation),
            "spawnBot": spawn_bot_json,
        }));
    }
    items
}

/// Dump companions (sidekicks and similar persistent entities).
///
/// The game also maintains a `game._-c1S` vector, but it is rebuilt during
/// player setup and can be empty at the instant `dump_state` runs. Walking the
/// live player entities and reading each entity's `_-i1o` Companion is stable
/// across those rebuilds.
fn dump_companions<'gc>(
    entities: Value<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> Vec<serde_json::Value> {
    let mut companions = Vec::new();
    let Some(object) = entities.as_object() else {
        return companions;
    };
    let Some(vector) = object.as_vector_storage() else {
        return companions;
    };
    let list: Vec<Value<'gc>> = vector.iter().collect();
    drop(vector);

    for value in list {
        let Some(entity_obj) = value.as_object() else {
            continue;
        };
        let entity = Value::from(entity_obj);

        // Cosmetic companion/pet (Companion).
        let companion_value = state_prop(entity, "_-i1o", activation);
        if let Some(companion_obj) = companion_value.as_object() {
            let companion = Value::from(companion_obj);
            let parent = state_prop(companion, "_-03X", activation);
            let companion_type = state_prop(companion, "_-n4x", activation);
            let kind = state_prop(companion_type, "_-DQ", activation)
                .coerce_to_string(activation)
                .map(|value| value.to_utf8_lossy().to_string())
                .unwrap_or_else(|_| "companion".to_string());
            companions.push(serde_json::json!({
                "x": state_num(state_prop(companion, "_-a34", activation), activation),
                "y": state_num(state_prop(companion, "_-J5u", activation), activation),
                "parentId": state_num(state_prop(parent, "_-H4y", activation), activation),
                "kind": kind,
            }));
        }

        // Sidekick/spawn bot carried by the player. This is the visible
        // companion in most replays even when the Companion object is not
        // instantiated by replay playback.
        let spawn_bot_value = state_prop(entity, "_-o1G", activation);
        if let Some(spawn_bot_obj) = spawn_bot_value.as_object() {
            let spawn_bot = Value::from(spawn_bot_obj);
            let parent = state_prop(spawn_bot, "_-wV", activation);
            companions.push(serde_json::json!({
                "x": state_num(state_prop(spawn_bot, "_-a34", activation), activation),
                "y": state_num(state_prop(spawn_bot, "_-J5u", activation), activation),
                "parentId": state_num(state_prop(parent, "_-H4y", activation), activation),
                "kind": "spawnbot",
            }));
        }
    }
    companions
}

/// Minimal 11.00 state dump.
///
/// The 11.00 live names are not the 10.10 names. This first pass emits the
/// player transform/animation fields the renderer needs; items, companions and
/// moving platforms can be added once the 11.00 token map is exercised.
fn dump_state_11000<'gc>(
    activation: &mut Activation<'_, 'gc>,
    game: Value<'gc>,
    supplied: Value<'gc>,
    time: u32,
) -> Result<Value<'gc>, Error<'gc>> {
    #[cfg(not(target_family = "wasm"))]
    if std::env::var("RUFFLE_STATE_FIXED_TIME")
        .map(|value| value != "0")
        .unwrap_or(false)
    {
        use std::sync::atomic::Ordering;
        if time > 0 {
            STATE_TIME_SEEN_POSITIVE.store(true, Ordering::Relaxed);
        }
        if !STATE_TIME_STARTED.load(Ordering::Relaxed) {
            // BeginReplay resets `mServerTimeThisTick` to 0. Anchor frame 0 to
            // that reset so a slow boot cannot shift the capture window.
            if time == 0 && STATE_TIME_SEEN_POSITIVE.load(Ordering::Relaxed) {
                STATE_TIME_STARTED.store(true, Ordering::Relaxed);
                DUMP_STATE_FRAME.store(0, Ordering::Relaxed);
                let real_ms = web_time::Instant::now()
                    .duration_since(activation.context.start_time)
                    .as_millis() as u32;
                STATE_FIXED_BASE_MS.store(real_ms, Ordering::Relaxed);
                crate::reset_dump_frame();
            } else {
                return Ok(Value::Undefined);
            }
        }
    }
    let entities = if supplied
        .as_object()
        .map(|object| object.as_vector_storage().is_some())
        .unwrap_or(false)
    {
        supplied
    } else {
        find_entities(game, activation)
    };
    let mut players = Vec::new();
    let mut companions = Vec::new();
    if let Some(object) = entities.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let items: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for item in items {
                let Some(entity) = item.as_object() else {
                    continue;
                };
                let entity = Value::from(entity);
                let gfx_type = state_prop(entity, "_-M6X", activation);
                let combat = state_prop(entity, "_-Kc", activation);
                let mut power_id = 0.0;
                let mut power_name = String::new();
                let mut power_frame = 0.0;
                if combat.as_object().is_some() {
                    let active = state_prop(combat, "_-o3I", activation);
                    if active.as_object().is_some() {
                        power_frame = state_num(state_prop(active, "_-c2l", activation), activation);
                        let power_type = state_prop(active, "_-V6B", activation);
                        if power_type.as_object().is_some() {
                            power_id =
                                state_num(state_prop(power_type, "_-H4O", activation), activation);
                            power_name =
                                state_str(state_prop(power_type, "_-r1e", activation), activation);
                        }
                    }
                }
                let anim_class = if gfx_type.as_object().is_some() {
                    state_str(state_prop(gfx_type, "_-F1J", activation), activation)
                } else {
                    String::new()
                };
                let run_anim = if gfx_type.as_object().is_some() {
                    state_num(state_prop(gfx_type, "_-f6B", activation), activation)
                } else {
                    0.0
                };
                let mut anim_name = String::new();
                let mut move_index = 0.0;
                let mut move_name = String::new();
                let mut frame_index = 0.0;
                let super_anim = state_prop(entity, "_-Vh", activation);
                if super_anim.as_object().is_some() {
                    let seq = state_prop(super_anim, "_-E2H", activation);
                    if seq.as_object().is_some() {
                        let anim_def = state_prop(seq, "_-E1X", activation);
                        if anim_def.as_object().is_some() {
                            anim_name =
                                state_str(state_prop(anim_def, "_-K1j", activation), activation);
                        }
                        let current_move = state_prop(seq, "_-v1Y", activation);
                        if current_move.as_object().is_some() {
                            move_index =
                                state_num(state_prop(current_move, "_-r3Y", activation), activation);
                            move_name =
                                state_str(state_prop(current_move, "_-012", activation), activation);
                        }
                        frame_index = state_num(state_prop(seq, "_-K5u", activation), activation);
                    }
                }
                let player = serde_json::json!({
                    "id": state_num(state_prop(entity, "_-11", activation), activation),
                    "name": state_str(state_prop(entity, "_-z33", activation), activation),
                    "x": state_num(call_state(entity, "_-p3V", activation), activation),
                    "y": state_num(call_state(entity, "_-Sb", activation), activation),
                    "centerX": state_num(call_state(entity, "_-p5a", activation), activation),
                    "centerY": state_num(call_state(entity, "_-d5F", activation), activation),
                    "vx": state_num(call_state(entity, "_-a3w", activation), activation),
                    "vy": state_num(call_state(entity, "_-M6G", activation), activation),
                    "hp": state_num(call_state(entity, "_-xE", activation), activation),
                    "state": state_num(state_prop(entity, "_-65w", activation), activation),
                    "facingLeft": state_bool(call_state(entity, "_-r35", activation)),
                    "dodgeTime": state_num(state_prop(entity, "_-H2b", activation), activation),
                    "dodgeDir": state_num(state_prop(entity, "_-O5C", activation), activation),
                    "dodgeType": state_num(state_prop(entity, "_-Q42", activation), activation),
                    "dashing": state_bool(state_prop(entity, "_-P50", activation)),
                    "postDash": state_bool(state_prop(entity, "_-M3V", activation)),
                    "dashId": state_num(state_prop(entity, "_-L3a", activation), activation),
                    "powerId": power_id,
                    "powerName": power_name,
                    "powerFrame": power_frame,
                    "animClass": anim_class,
                    "runAnim": run_anim,
                    "animName": anim_name,
                    "moveIndex": move_index,
                    "moveName": move_name,
                    "frameIndex": frame_index,
                });
                players.push(player);

                let spawn_bot = state_prop(entity, "_-k5S", activation);
                if spawn_bot.as_object().is_some() {
                    companions.push(serde_json::json!({
                        "x": state_num(state_prop(spawn_bot, "_-J2c", activation), activation),
                        "y": state_num(state_prop(spawn_bot, "_-b3h", activation), activation),
                        "parentId": state_num(state_prop(entity, "_-11", activation), activation),
                        "kind": "spawnbot",
                    }));
                }
            }
        }
    }

    let mut items = Vec::new();
    let item_manager = state_prop(game, "_-3z", activation);
    let world_items = state_prop(item_manager, "_-O1L", activation);
    if let Some(object) = world_items.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let values: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for value in values {
                let Some(item_obj) = value.as_object() else {
                    continue;
                };
                let item = Value::from(item_obj);
                let data = state_prop(item, "_-x5F", activation);
                let item_type = state_prop(item, "_-93n", activation);
                let spawn_bot = state_prop(item, "_-k5S", activation);
                let name = if item_type.as_object().is_some() {
                    state_str(state_prop(item_type, "_-E49", activation), activation)
                } else {
                    String::new()
                };
                let spawn_bot_json = if spawn_bot.as_object().is_some() {
                    serde_json::json!({
                        "x": state_num(state_prop(spawn_bot, "_-J2c", activation), activation),
                        "y": state_num(state_prop(spawn_bot, "_-b3h", activation), activation),
                        "state": state_num(state_prop(spawn_bot, "_-T1X", activation), activation),
                        "parentId": state_num(state_prop(spawn_bot, "_-767", activation), activation),
                    })
                } else {
                    serde_json::Value::Null
                };
                items.push(serde_json::json!({
                    "id": state_num(state_prop(data, "_-H2Z", activation), activation),
                    "name": name,
                    "x": state_num(call_state(item, "_-p3V", activation), activation),
                    "y": state_num(call_state(item, "_-Sb", activation), activation),
                    "vx": state_num(state_prop(item, "_-Lu", activation), activation),
                    "vy": state_num(state_prop(item, "_-33c", activation), activation),
                    "state": state_num(state_prop(item, "_-r5B", activation), activation),
                    "team": state_num(state_prop(item, "_-25G", activation), activation),
                    "thrownBy": state_num(state_prop(item, "_-x5G", activation), activation),
                    "thrownByTeam": state_num(state_prop(item, "_-h2B", activation), activation),
                    "thrownTime": state_num(state_prop(item, "_-O13", activation), activation),
                    "spawnTick": state_num(state_prop(item, "_-b37", activation), activation),
                    "createdAt": state_num(state_prop(data, "_-D3Q", activation), activation),
                    "owner": state_num(state_prop(data, "_-n3E", activation), activation),
                    "lastHolder": state_num(state_prop(data, "_-P6L", activation), activation),
                    "stuck": state_bool(state_prop(item, "_-f4i", activation)),
                    "tossed": state_bool(state_prop(item, "_-U5y", activation)),
                    "spawnBot": spawn_bot_json,
                }));
            }
        }
    }

    let mut platforms = Vec::new();
    let scene_manager = state_prop(game, "_-22K", activation);
    let moving_platforms = state_prop(scene_manager, "_-c5B", activation);
    if let Some(object) = moving_platforms.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let values: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for value in values {
                let Some(platform_obj) = value.as_object() else {
                    continue;
                };
                let platform = Value::from(platform_obj);
                platforms.push(serde_json::json!({
                    "id": state_str(state_prop(platform, "_-94r", activation), activation),
                    "x": state_num(state_prop(platform, "_-G6H", activation), activation),
                    "y": state_num(state_prop(platform, "_-O6b", activation), activation),
                    "disabled": state_bool(state_prop(platform, "_-H2e", activation)),
                }));
            }
        }
    }

    let game_companions = state_prop(game, "_-O3r", activation);
    if let Some(object) = game_companions.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let values: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for value in values {
                let Some(companion_obj) = value.as_object() else {
                    continue;
                };
                let companion = Value::from(companion_obj);
                companions.push(serde_json::json!({
                    "x": state_num(state_prop(companion, "_-J2c", activation), activation),
                    "y": state_num(state_prop(companion, "_-b3h", activation), activation),
                    "parentId": 0,
                    "kind": "companion",
                }));
            }
        }
    }

    let line = serde_json::json!({
        "frame": 0,
        "t": time,
        "gameId": 0,
        "platforms": platforms,
        "players": players,
        "items": items,
        "companions": companions,
        "boxes": [],
    });

    #[cfg(not(target_family = "wasm"))]
    {
        let frame = DUMP_STATE_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::var("RUFFLE_STATE_PATH").unwrap_or_else(|_| "states.jsonl".to_string());
        let file = STATE_FILE.get_or_init(|| {
            Mutex::new(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .expect("open RUFFLE_STATE_PATH"),
            )
        });
        if let Ok(mut file) = file.lock() {
            let mut line = line;
            // Box rows dumped later in this tick read `DUMP_FRAME`, so keep it
            // aligned with the state frame that just started.
            crate::DUMP_FRAME.store(frame, std::sync::atomic::Ordering::Relaxed);
            if std::env::var("RUFFLE_STATE_FIXED_TIME")
                .map(|value| value != "0")
                .unwrap_or(false)
            {
                let tick_ms = std::env::var("RUFFLE_STATE_TICK_MS")
                    .ok()
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(16);
                let origin = std::env::var("RUFFLE_STATE_ORIGIN_T")
                    .ok()
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(0);
                line["t"] = serde_json::json!(origin + (frame as i64) * tick_ms);
            }
            line["frame"] = serde_json::json!(frame);
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
        dump_debug_11000(activation, game, entities, frame, time);
    }

    #[cfg(target_family = "wasm")]
    {
        let frame = crate::DUMP_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut line = line;
        line["frame"] = serde_json::json!(frame);
        crate::push_state_line(line.to_string());
    }

    Ok(Value::Undefined)
}

/// Dump one frame of per-player gameplay state as JSONL.
pub fn dump_state<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    if !REPLAY_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(Value::Undefined);
    }
    let game = args.get_value(0);
    let supplied = args.get_value(1);
    let supplied_is_vector = supplied
        .as_object()
        .map(|o| o.as_vector_storage().is_some())
        .unwrap_or(false);
    let entities = if supplied_is_vector {
        supplied
    } else {
        find_entities(game, activation)
    };
    let time = args.get_u32(2);
    if BUILD_11000.load(std::sync::atomic::Ordering::Relaxed) {
        return dump_state_11000(activation, game, supplied, time);
    }
    let mut platforms = Vec::new();
    let scene_manager = state_prop(game, "_-o3i", activation);
    let moving_platforms = state_prop(scene_manager, "_-046", activation);
    if let Some(object) = moving_platforms.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let items: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for item in items {
                let Some(platform) = item.as_object() else {
                    continue;
                };
                let platform = Value::from(platform);
                platforms.push(serde_json::json!({
                    "id": state_prop(platform, "_-N6l", activation)
                        .coerce_to_string(activation)
                        .map(|s| s.to_utf8_lossy().to_string())
                        .unwrap_or_default(),
                    "x": state_num(state_prop(platform, "_-J2M", activation), activation),
                    "y": state_num(state_prop(platform, "_-M1r", activation), activation),
                    "disabled": state_bool(state_prop(platform, "_-T3C", activation)),
                }));
            }
        }
    }
    let mut players = Vec::new();
    if let Some(object) = entities.as_object() {
        if let Some(vector) = object.as_vector_storage() {
            let items: Vec<Value<'gc>> = vector.iter().collect();
            drop(vector);
            for item in items {
                let Some(entity) = item.as_object() else {
                    continue;
                };
                let entity = Value::from(entity);
                let name = state_prop(entity, "_-237", activation)
                    .coerce_to_string(activation)
                    .map(|s| s.to_utf8_lossy().to_string())
                    .unwrap_or_default();
                let combat = state_prop(entity, "_-33w", activation);
                let gfx = state_prop(entity, "_-X3B", activation);
                let seq = state_prop(gfx, "_-h3g", activation);
                let anim_def = state_prop(seq, "_-J4A", activation);
                let anim_name = state_prop(anim_def, "_-u4y", activation)
                    .coerce_to_string(activation)
                    .map(|s| s.to_utf8_lossy().to_string())
                    .unwrap_or_default();
                let current_move = state_prop(seq, "_-T5L", activation);
                let gfx_type = state_prop(entity, "_-z46", activation);
                let anim_class = state_prop(gfx_type, "_-G5y", activation)
                    .coerce_to_string(activation)
                    .map(|s| s.to_utf8_lossy().to_string())
                    .unwrap_or_default();
                let run_anim = state_num(state_prop(gfx_type, "_-j5T", activation), activation);
                let move_index = state_num(state_prop(current_move, "_-83a", activation), activation);
                let move_name = state_prop(current_move, "_-x4K", activation)
                    .coerce_to_string(activation)
                    .map(|s| s.to_utf8_lossy().to_string())
                    .unwrap_or_default();
                // Seq.mCurrentFrameIndex is the live frame cursor. The old
                // mCurrentFrame.mFrameIndex value was the frame's index inside
                // its source clip and stayed at zero for most moves.
                let frame_index = state_num(state_prop(seq, "_-E5B", activation), activation);
                let mut power_id = 0.0;
                let mut power_name = String::new();
                let mut power_frame = 0.0;
                if let Some(combat_obj) = combat.as_object() {
                    let combat = Value::from(combat_obj);
                    let active = state_prop(combat, "_-f5t", activation);
                    if let Some(active_obj) = active.as_object() {
                        let active = Value::from(active_obj);
                        power_frame = state_num(
                            state_prop(active, "_-b1w", activation),
                            activation,
                        );
                        let power_type = state_prop(active, "_-fA", activation);
                        if let Some(power_obj) = power_type.as_object() {
                            let power_type = Value::from(power_obj);
                            power_id = state_num(
                                state_prop(power_type, "_-Q5D", activation),
                                activation,
                            );
                            power_name = state_prop(power_type, "_-V1q", activation)
                                .coerce_to_string(activation)
                                .map(|s| s.to_utf8_lossy().to_string())
                                .unwrap_or_default();
                        }
                    }
                }
                let player = serde_json::json!({
                    "id": state_num(state_prop(entity, "_-H4y", activation), activation),
                    "name": name,
                    "x": state_num(call_state(entity, "_-96p", activation), activation),
                    "y": state_num(call_state(entity, "_-W2z", activation), activation),
                    "centerX": state_num(state_prop(entity, "_-sz", activation), activation),
                    "centerY": state_num(state_prop(entity, "_-5Z", activation), activation),
                    "vx": state_num(call_state(entity, "_-v5x", activation), activation),
                    "vy": state_num(call_state(entity, "_-y1Z", activation), activation),
                    "hp": state_num(state_prop(entity, "_-M4z", activation), activation),
                    "state": state_num(state_prop(entity, "_-B1u", activation), activation),
                    // mLeftFacingBit is a packed bitfield; call the accessor
                    // instead of treating the raw uint as a boolean.
                    "facingLeft": state_bool(call_state(entity, "_-92j", activation)),
                    "dodgeTime": state_num(state_prop(entity, "_-S2S", activation), activation),
                    "dodgeDir": state_num(state_prop(entity, "_-n2v", activation), activation),
                    "dodgeType": state_num(state_prop(entity, "_-f1h", activation), activation),
                    "dashing": state_bool(state_prop(entity, "_-MZ", activation)),
                    "postDash": state_bool(state_prop(entity, "_-h5o", activation)),
                    "dashId": state_num(state_prop(entity, "_-oE", activation), activation),
                    "powerId": power_id,
                    "powerName": power_name,
                    "powerFrame": power_frame,
                    "animClass": anim_class,
                    "runAnim": run_anim,
                    "animName": anim_name,
                    "moveIndex": move_index,
                    "moveName": move_name,
                    "frameIndex": frame_index,
                });
                players.push(player);
            }
        }
    }

    let items = dump_items(game, activation);
    let companions = dump_companions(entities, activation);

    #[cfg(not(target_family = "wasm"))]
    {
        let frame = DUMP_STATE_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::var("RUFFLE_STATE_PATH")
            .unwrap_or_else(|_| "states.jsonl".to_string());
        let file = STATE_FILE.get_or_init(|| {
            Mutex::new(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .expect("open RUFFLE_STATE_PATH"),
            )
        });
        if let Ok(mut file) = file.lock() {
            let line = serde_json::json!({
                "frame": frame,
                "t": time,
                "gameId": state_num(state_prop(game, "_-q4R", activation), activation),
                "platforms": platforms,
                "players": players,
                "items": items,
                "companions": companions,
            });
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
    }

    // Fast extraction: re-arm the game's own frame-step counter after every
    // captured frame. The game then processes `batch` replay frames per outer
    // tick and calls dump_state once per frame.
    #[cfg(not(target_family = "wasm"))]
    {
        let batch = std::env::var("RUFFLE_REPLAY_BATCH")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1);
        if batch > 1 {
            let _ = game.call_public_property(
                AvmString::new_utf8(activation.gc(), "_-B3K"),
                FunctionArgs::from_slice(&[Value::Number(0.0)]),
                activation,
            );
            let _ = game.set_public_property(
                AvmString::new_utf8(activation.gc(), "_-83e"),
                Value::Number(batch as f64),
                activation,
            );
        }
    }

    #[cfg(target_family = "wasm")]
    {
        let frame = crate::DUMP_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let line = serde_json::json!({
            "frame": frame,
            "t": time,
            "gameId": state_num(state_prop(game, "_-q4R", activation), activation),
            "platforms": platforms,
            "players": players,
            "items": items,
            "companions": companions,
        });
        crate::push_state_line(line.to_string());
    }

    Ok(Value::Undefined)
}

/// Reset the native frame counter when a replay starts.
pub fn reset_dump_frame<'gc>(
    _activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    _args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    crate::reset_dump_frame();
    Ok(Value::Undefined)
}

pub fn get_command_line<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    _args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    #[cfg(not(target_family = "wasm"))]
    let args = std::env::var("RUFFLE_SWF_ARGS").unwrap_or_default();
    #[cfg(target_family = "wasm")]
    let args = "-forceoffline".to_string();
    Ok(AvmString::new_utf8(activation.gc(), args).into())
}

pub fn is_dependent<'gc>(
    _activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    if let Some(s) = args.try_get_string(0) {
        return Ok(s.is_dependent().into());
    }

    panic!();
}

fn string_arg<'gc>(
    activation: &mut Activation<'_, 'gc>,
    args: &FunctionArgs<'_, 'gc>,
    index: usize,
) -> String {
    args.get_string(activation, index)
        .to_utf8_lossy()
        .to_string()
}

/// `FileStream` support: read an entire file as a `ByteArray`, or `null` if missing.
pub fn fs_read<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let path = string_arg(activation, &args, 0);
    tracing::debug!("AIR fs_read {path}");
    let result = activation.context.filesystem.read(&path);
    match result {
        Ok(bytes) => {
            let storage = ByteArrayStorage::from_vec(activation.context, bytes);
            Ok(ByteArrayObject::from_storage(activation.context, storage).into())
        }
        Err(err) => {
            tracing::warn!("AIR fs_read failed for {path:?}: {err}");
            Ok(Value::Null)
        }
    }
}

pub fn fs_write<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let path = string_arg(activation, &args, 0);
    let Some(bytes) = args.try_get_object(1).and_then(|object| {
        object
            .as_bytearray()
            .map(|storage| storage.bytes().to_vec())
    }) else {
        return Ok(false.into());
    };
    Ok(activation
        .context
        .filesystem
        .write(&path, &bytes)
        .map(|()| {
            tracing::debug!("AIR fs_write {path} ({} bytes)", bytes.len());
            true
        })
        .unwrap_or_else(|err| {
            tracing::warn!("AIR fs_write failed for {path:?}: {err}");
            false
        })
        .into())
}

pub fn fs_exists<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let path = string_arg(activation, &args, 0);
    tracing::debug!("AIR fs_exists {path}");
    Ok(activation.context.filesystem.exists(&path).into())
}

pub fn fs_is_directory<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let path = string_arg(activation, &args, 0);
    Ok(activation.context.filesystem.is_directory(&path).into())
}

pub fn fs_size<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let path = string_arg(activation, &args, 0);
    Ok(activation
        .context
        .filesystem
        .size(&path)
        .map(|size| size as f64)
        .unwrap_or(-1.0)
        .into())
}

/// Returns an `Array` of entry names (not paths).
pub fn fs_list_directory<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let path = string_arg(activation, &args, 0);
    let result = activation.context.filesystem.list_directory(&path);
    match result {
        Ok(entries) => {
            let names: Vec<Value<'gc>> = entries
                .into_iter()
                .map(|entry| AvmString::new_utf8(activation.gc(), entry.name).into())
                .collect();
            Ok(ArrayObject::from_storage(
                activation.context,
                ArrayStorage::from_iter(names),
            )
            .into())
        }
        Err(_) => Ok(Value::Null),
    }
}

pub fn fs_create_directory<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let path = string_arg(activation, &args, 0);
    Ok(activation
        .context
        .filesystem
        .create_directory(&path)
        .is_ok()
        .into())
}

pub fn fs_delete<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let path = string_arg(activation, &args, 0);
    Ok(activation.context.filesystem.delete(&path).is_ok().into())
}

/// `nativePath` roots exposed to `flash.filesystem.File`.
pub fn fs_directory<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let which = string_arg(activation, &args, 0);
    let filesystem = &*activation.context.filesystem;
    let path = match which.as_str() {
        "application" => filesystem.application_directory(),
        "storage" => filesystem.application_storage_directory(),
        "user" => filesystem.user_directory(),
        "documents" => filesystem.documents_directory(),
        "desktop" => filesystem.desktop_directory(),
        _ => String::new(),
    };
    Ok(AvmString::new_utf8(activation.gc(), path).into())
}

/// Decode a Brawlhalla `.swz` container into an `Array` of UTF-8 entry strings.
///
/// Returns `null` when the container is malformed or the key is wrong (matching
/// a failed `RawData_SetData` call in AIR).
pub fn swz_decode<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Value<'gc>,
    args: FunctionArgs<'_, 'gc>,
) -> Result<Value<'gc>, Error<'gc>> {
    let Some(bytes) = args.try_get_object(0).and_then(|object| {
        object
            .as_bytearray()
            .map(|storage| storage.bytes().to_vec())
    }) else {
        return Ok(Value::Null);
    };
    let key = args.get_u32(1);

    match crate::swz::decode(&bytes, key) {
        Ok(entries) => {
            tracing::warn!("swz_decode: {} entries", entries.len());
            let values: Vec<Value<'gc>> = entries
                .into_iter()
                .map(|entry| AvmString::new_utf8(activation.gc(), entry).into())
                .collect();
            Ok(ArrayObject::from_storage(
                activation.context,
                ArrayStorage::from_iter(values),
            )
            .into())
        }
        Err(err) => {
            tracing::warn!("Failed to decode SWZ container: {err}");
            Ok(Value::Null)
        }
    }
}
