/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/LanRhyme/MicYou>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version, with the MicYou Plugin Exception.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 */

//! macOS far-end reference capture through Core Audio process taps.
//!
//! macOS has no WASAPI-style loopback: before 14.2 the only capture sources are
//! device inputs, which makes a speaker reference impossible without a virtual
//! device MicYou itself is already writing to. `AudioHardwareCreateProcessTap`
//! (macOS 14.2+) taps what the system is playing while excluding MicYou's own
//! process, which is the platform equivalent of the Windows/Linux capture paths.
//!
//! The Core Audio functions are resolved with `dlopen`/`dlsym` instead of being
//! linked. A linked reference to `AudioHardwareCreateProcessTap` would leave an
//! unresolved symbol on older systems, and resolving it at run time doubles as
//! the capability probe that `crate::aec` reports to every frontend.

// `class!`/`msg_send!` expand into code that tests a `cargo-clippy` cfg this
// crate does not declare, which the compiler otherwise reports per expansion.
#![allow(unexpected_cfgs)]

use std::ffi::c_void;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use objc::runtime::{Class, Object};
use objc::{class, msg_send, sel, sel_impl};
use ringbuf::HeapRb;

use crate::loopback::{lock, push_to_buffer, set_failure};
use crate::AecFailure;

const CORE_AUDIO_PATH: &[u8] = b"/System/Library/Frameworks/CoreAudio.framework/CoreAudio\0";
const RTLD_NOW: c_int = 2;

/// `AudioHardwareCreateProcessTap`, the symbol that separates 14.2+ from older releases.
pub(crate) const SYMBOL_CREATE_PROCESS_TAP: &[u8] = b"AudioHardwareCreateProcessTap\0";

const SYMBOL_DESTROY_PROCESS_TAP: &[u8] = b"AudioHardwareDestroyProcessTap\0";
const SYMBOL_CREATE_AGGREGATE_DEVICE: &[u8] = b"AudioHardwareCreateAggregateDevice\0";
const SYMBOL_DESTROY_AGGREGATE_DEVICE: &[u8] = b"AudioHardwareDestroyAggregateDevice\0";
const SYMBOL_GET_PROPERTY_DATA: &[u8] = b"AudioObjectGetPropertyData\0";
const SYMBOL_CREATE_IO_PROC: &[u8] = b"AudioDeviceCreateIOProcID\0";
const SYMBOL_START_DEVICE: &[u8] = b"AudioDeviceStart\0";
const SYMBOL_STOP_DEVICE: &[u8] = b"AudioDeviceStop\0";
const SYMBOL_DESTROY_IO_PROC: &[u8] = b"AudioDeviceDestroyIOProcID\0";

const USAGE_DESCRIPTION_KEY: &[u8] = b"NSAudioCaptureUsageDescription\0";

extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

/// The Core Audio framework handle, opened once per process and never closed.
///
/// The handle is stored as an address because raw pointers are neither `Send`
/// nor `Sync` and therefore cannot live in a `OnceLock`.
fn framework_handle() -> Option<*mut c_void> {
    static HANDLE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let address = *HANDLE.get_or_init(|| unsafe {
        dlopen(CORE_AUDIO_PATH.as_ptr() as *const c_char, RTLD_NOW) as usize
    });
    if address == 0 {
        None
    } else {
        Some(address as *mut c_void)
    }
}

/// Resolves a NUL-terminated Core Audio symbol name.
///
/// Returns `None` when the framework cannot be opened or the running system does
/// not export the symbol.
pub(crate) fn resolve(symbol: &[u8]) -> Option<*mut c_void> {
    let handle = framework_handle()?;
    let resolved = unsafe { dlsym(handle, symbol.as_ptr() as *const c_char) };
    if resolved.is_null() {
        None
    } else {
        Some(resolved)
    }
}

/// Whether this system can create a Core Audio process tap.
pub(crate) fn process_tap_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| resolve(SYMBOL_CREATE_PROCESS_TAP).is_some())
}

fn symbol<T>(name: &[u8]) -> Option<T> {
    let pointer = resolve(name)?;
    Some(unsafe { std::mem::transmute_copy(&pointer) })
}

// ─── Core Audio ABI ────────────────────────────────────────────────────────

type OSStatus = i32;
type AudioObjectID = u32;

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[repr(C)]
struct AudioBuffer {
    number_channels: u32,
    data_byte_size: u32,
    data: *mut c_void,
}

#[repr(C)]
struct AudioBufferList {
    number_buffers: u32,
    buffers: [AudioBuffer; 1],
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

type IoProc = unsafe extern "C" fn(
    AudioObjectID,
    *const c_void,
    *const AudioBufferList,
    *const c_void,
    *mut AudioBufferList,
    *const c_void,
    *mut c_void,
) -> OSStatus;

const fn fourcc(chars: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*chars)
}

const SYSTEM_OBJECT: AudioObjectID = 1;
const SCOPE_GLOBAL: u32 = fourcc(b"glob");
const ELEMENT_MAIN: u32 = 0;
const PROPERTY_TRANSLATE_PID: u32 = fourcc(b"id2p");
const PROPERTY_TAP_FORMAT: u32 = fourcc(b"tfmt");

/// Core Audio entry points a process tap needs, resolved once per capture.
struct CoreAudioApi {
    create_process_tap: unsafe extern "C" fn(*mut Object, *mut AudioObjectID) -> OSStatus,
    destroy_process_tap: unsafe extern "C" fn(AudioObjectID) -> OSStatus,
    create_aggregate_device: unsafe extern "C" fn(*const c_void, *mut AudioObjectID) -> OSStatus,
    destroy_aggregate_device: unsafe extern "C" fn(AudioObjectID) -> OSStatus,
    get_property_data: unsafe extern "C" fn(
        AudioObjectID,
        *const AudioObjectPropertyAddress,
        u32,
        *const c_void,
        *mut u32,
        *mut c_void,
    ) -> OSStatus,
    create_io_proc: unsafe extern "C" fn(
        AudioObjectID,
        IoProc,
        *mut c_void,
        *mut *mut c_void,
    ) -> OSStatus,
    start_device: unsafe extern "C" fn(AudioObjectID, *mut c_void) -> OSStatus,
    stop_device: unsafe extern "C" fn(AudioObjectID, *mut c_void) -> OSStatus,
    destroy_io_proc: unsafe extern "C" fn(AudioObjectID, *mut c_void) -> OSStatus,
}

impl CoreAudioApi {
    fn load() -> Option<Self> {
        Some(Self {
            create_process_tap: symbol(SYMBOL_CREATE_PROCESS_TAP)?,
            destroy_process_tap: symbol(SYMBOL_DESTROY_PROCESS_TAP)?,
            create_aggregate_device: symbol(SYMBOL_CREATE_AGGREGATE_DEVICE)?,
            destroy_aggregate_device: symbol(SYMBOL_DESTROY_AGGREGATE_DEVICE)?,
            get_property_data: symbol(SYMBOL_GET_PROPERTY_DATA)?,
            create_io_proc: symbol(SYMBOL_CREATE_IO_PROC)?,
            start_device: symbol(SYMBOL_START_DEVICE)?,
            stop_device: symbol(SYMBOL_STOP_DEVICE)?,
            destroy_io_proc: symbol(SYMBOL_DESTROY_IO_PROC)?,
        })
    }

    /// Translates a process id into the Core Audio process object a tap excludes.
    fn process_object(&self, pid: u32) -> Option<AudioObjectID> {
        let address = AudioObjectPropertyAddress {
            selector: PROPERTY_TRANSLATE_PID,
            scope: SCOPE_GLOBAL,
            element: ELEMENT_MAIN,
        };
        let mut object: AudioObjectID = 0;
        let mut size = std::mem::size_of::<AudioObjectID>() as u32;
        let status = unsafe {
            (self.get_property_data)(
                SYSTEM_OBJECT,
                &address,
                std::mem::size_of::<u32>() as u32,
                &pid as *const u32 as *const c_void,
                &mut size,
                &mut object as *mut AudioObjectID as *mut c_void,
            )
        };
        if status == 0 {
            Some(object)
        } else {
            None
        }
    }

    fn tap_format(&self, tap: AudioObjectID) -> Option<AudioStreamBasicDescription> {
        let address = AudioObjectPropertyAddress {
            selector: PROPERTY_TAP_FORMAT,
            scope: SCOPE_GLOBAL,
            element: ELEMENT_MAIN,
        };
        let mut format = AudioStreamBasicDescription::default();
        let mut size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
        let status = unsafe {
            (self.get_property_data)(
                tap,
                &address,
                std::mem::size_of::<u32>() as u32,
                std::ptr::null(),
                &mut size,
                &mut format as *mut AudioStreamBasicDescription as *mut c_void,
            )
        };
        if status == 0 {
            Some(format)
        } else {
            None
        }
    }
}

// ─── Capture thread ───────────────────────────────────────────────────────

/// Resampler state shared with the real-time IOProc callback.
///
/// The rate and the resampler built for it are swapped together: following a
/// device switch must never leave the callback resampling at the previous rate.
/// A `None` resampler means the tap already runs at the target rate.
struct TapSource {
    device_rate: u32,
    resampler: Option<Arc<Mutex<crate::engine::RubatoResampler>>>,
}

/// State shared with the real-time IOProc callback.
struct TapContext {
    buffer: Arc<Mutex<HeapRb<f32>>>,
    source: Mutex<TapSource>,
    channels: usize,
}

unsafe extern "C" fn tap_io_proc(
    _device: AudioObjectID,
    _now: *const c_void,
    input: *const AudioBufferList,
    _input_time: *const c_void,
    _output: *mut AudioBufferList,
    _output_time: *const c_void,
    client: *mut c_void,
) -> OSStatus {
    if client.is_null() || input.is_null() {
        return 0;
    }
    unsafe {
        let context = &*(client as *const TapContext);
        // Clone the resampler out of the lock: resampling happens outside it, so a
        // rate change on the supervisor thread can swap it mid-callback.
        let source = lock(&context.source);
        let device_rate = source.device_rate;
        let resampler = source.resampler.clone();
        drop(source);
        let list = &*input;
        for index in 0..list.number_buffers as usize {
            let audio_buffer = &list.buffers[index];
            if audio_buffer.data.is_null() || audio_buffer.data_byte_size == 0 {
                continue;
            }
            let count = audio_buffer.data_byte_size as usize / 4;
            let samples = std::slice::from_raw_parts(audio_buffer.data as *const f32, count);
            push_to_buffer(
                samples,
                context.channels,
                device_rate,
                &resampler,
                &context.buffer,
            );
        }
    }
    0
}

/// Whether the running bundle declares why it records system audio.
///
/// macOS refuses the authorization request outright when the key is missing and
/// then feeds the tap silence, so this is checked up front to turn a silent
/// failure into an actionable one. A bare development binary has no bundle
/// metadata and therefore cannot capture at all. The answer is fixed for the
/// lifetime of the process, so it is part of the capability `crate::aec` caches.
pub(crate) fn bundle_declares_audio_capture() -> bool {
    // Looked up rather than `class!`, which panics when Foundation is not loaded.
    let (Some(bundle_class), Some(string_class)) =
        (Class::get("NSBundle"), Class::get("NSString"))
    else {
        return false;
    };
    unsafe {
        let bundle: *mut Object = msg_send![bundle_class, mainBundle];
        if bundle.is_null() {
            return false;
        }
        let key: *mut Object = msg_send![
            string_class,
            stringWithUTF8String: USAGE_DESCRIPTION_KEY.as_ptr() as *const c_char
        ];
        if key.is_null() {
            return false;
        }
        let value: *mut Object = msg_send![bundle, objectForInfoDictionaryKey: key];
        !value.is_null()
    }
}

/// Captures the system playback as AEC far-end reference until `active` clears.
pub(crate) fn capture_thread(
    active: Arc<AtomicBool>,
    buffer: Arc<Mutex<HeapRb<f32>>>,
    failure: Arc<Mutex<Option<AecFailure>>>,
) {
    if let Err(reason) = run_capture(&active, &buffer) {
        log::error!(
            "[Loopback] Core Audio tap capture failed: {}",
            reason.as_str()
        );
        set_failure(&failure, reason);
    }
    active.store(false, Ordering::Relaxed);
}

/// Whether an observed tap rate requires rebuilding the resampler.
///
/// A zero rate means the format could not be read, which is ignored - matching the
/// check applied when the capture starts.
fn needs_resampler_rebuild(current_rate: u32, observed_rate: u32) -> bool {
    observed_rate > 0 && observed_rate != current_rate
}

/// Builds the resampler for a tap rate, or nothing when it already matches.
fn build_resampler(
    device_rate: u32,
) -> Result<Option<Arc<Mutex<crate::engine::RubatoResampler>>>, AecFailure> {
    if device_rate == crate::loopback::TARGET_RATE {
        return Ok(None);
    }
    match crate::engine::RubatoResampler::new(device_rate, crate::loopback::TARGET_RATE, 1) {
        Ok(resampler) => Ok(Some(Arc::new(Mutex::new(resampler)))),
        Err(error) => {
            log::error!("[Loopback] Failed to create a {device_rate}Hz resampler: {error}");
            Err(AecFailure::ReferenceLost)
        }
    }
}

fn run_capture(
    active: &Arc<AtomicBool>,
    buffer: &Arc<Mutex<HeapRb<f32>>>,
) -> Result<(), AecFailure> {
    let Some(api) = CoreAudioApi::load() else {
        log::error!(
            "[Loopback] Core Audio process taps are unavailable; macOS 14.2 or newer is required"
        );
        return Err(AecFailure::UnsupportedOs);
    };
    if !bundle_declares_audio_capture() {
        log::error!(
            "[Loopback] This bundle does not declare {}. System audio capture is refused, so \
             AEC has no far-end reference.",
            std::str::from_utf8(&USAGE_DESCRIPTION_KEY[..USAGE_DESCRIPTION_KEY.len() - 1])
                .unwrap_or("NSAudioCaptureUsageDescription")
        );
        return Err(AecFailure::PermissionDenied);
    }

    let Some(process_object) = api.process_object(std::process::id()) else {
        log::error!("[Loopback] Failed to translate the process id into a Core Audio object");
        return Err(AecFailure::ReferenceLost);
    };

    // Tapping every process except MicYou keeps the reference clean even when the
    // user routes the system output through a virtual device MicYou also writes to.
    let description: *mut Object = unsafe {
        let number: *mut Object =
            msg_send![class!(NSNumber), numberWithUnsignedInt: process_object];
        let processes: *mut Object = msg_send![class!(NSArray), arrayWithObject: number];
        let allocated: *mut Object = msg_send![class!(CATapDescription), alloc];
        let description: *mut Object =
            msg_send![allocated, initMonoGlobalTapButExcludeProcesses: processes];
        if !description.is_null() {
            let _: () = msg_send![description, setPrivate: true];
            // Unmuted: the user must still hear the original playback.
            let _: () = msg_send![description, setMuteBehavior: 0i64];
        }
        description
    };
    if description.is_null() {
        log::error!("[Loopback] Failed to build a CATapDescription");
        return Err(AecFailure::ReferenceLost);
    }

    let mut tap: AudioObjectID = 0;
    let status = unsafe { (api.create_process_tap)(description, &mut tap) };
    if status != 0 {
        log::error!("[Loopback] AudioHardwareCreateProcessTap failed: {status}");
        unsafe { release_description(description) };
        return Err(AecFailure::ReferenceLost);
    }
    log::info!("[Loopback] Core Audio process tap created");

    let format = api.tap_format(tap);
    let device_rate = format
        .map(|format| format.sample_rate.round() as u32)
        .filter(|rate| *rate > 0)
        .unwrap_or(crate::loopback::TARGET_RATE);
    let channels = format
        .map(|format| format.channels_per_frame as usize)
        .filter(|channels| *channels > 0)
        .unwrap_or(1);
    log::info!(
        "[Loopback] Core Audio tap format: {}Hz {}ch",
        device_rate,
        channels
    );

    let resampler = match build_resampler(device_rate) {
        Ok(resampler) => resampler,
        Err(reason) => {
            unsafe {
                (api.destroy_process_tap)(tap);
                release_description(description);
            }
            return Err(reason);
        }
    };

    let aggregate = match create_aggregate(&api, description) {
        Some(aggregate) => aggregate,
        None => {
            unsafe {
                (api.destroy_process_tap)(tap);
                release_description(description);
            }
            return Err(AecFailure::ReferenceLost);
        }
    };
    log::info!("[Loopback] Core Audio tap aggregate device created");

    let context = Box::into_raw(Box::new(TapContext {
        buffer: buffer.clone(),
        source: Mutex::new(TapSource {
            device_rate,
            resampler,
        }),
        channels,
    }));

    let mut io_proc: *mut c_void = std::ptr::null_mut();
    let created = unsafe {
        (api.create_io_proc)(aggregate, tap_io_proc, context as *mut c_void, &mut io_proc)
    };
    let started = if created == 0 {
        unsafe { (api.start_device)(aggregate, io_proc) }
    } else {
        created
    };
    if created != 0 || started != 0 {
        log::error!(
            "[Loopback] Failed to start the tap IOProc (create={created}, start={started})"
        );
        cleanup(&api, aggregate, io_proc, tap, description, context);
        return Err(AecFailure::ReferenceLost);
    }

    log::info!("[Loopback] Core Audio tap capture started");
    // The tap follows the default output device, so switching devices can change its
    // rate. Left alone, a stale ratio would misalign the reference silently, which is
    // worse than failing - so the rate is re-read here and the resampler rebuilt.
    let context_ref = unsafe { &*context };
    let mut failure = None;
    while active.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let observed_rate = api
            .tap_format(tap)
            .map(|format| format.sample_rate.round() as u32)
            .unwrap_or(0);
        let current_rate = lock(&context_ref.source).device_rate;
        if !needs_resampler_rebuild(current_rate, observed_rate) {
            continue;
        }
        match build_resampler(observed_rate) {
            Ok(resampler) => {
                *lock(&context_ref.source) = TapSource {
                    device_rate: observed_rate,
                    resampler,
                };
                log::info!("[Loopback] Core Audio tap rate changed to {observed_rate}Hz");
            }
            Err(reason) => {
                log::error!("[Loopback] Failed to follow the tap rate change");
                failure = Some(reason);
                break;
            }
        }
    }

    cleanup(&api, aggregate, io_proc, tap, description, context);
    log::info!("[Loopback] Core Audio tap capture stopped");
    match failure {
        Some(reason) => Err(reason),
        None => Ok(()),
    }
}

fn create_aggregate(api: &CoreAudioApi, description: *mut Object) -> Option<AudioObjectID> {
    unsafe {
        let uuid: *mut Object = msg_send![description, UUID];
        if uuid.is_null() {
            return None;
        }
        let uuid_string: *mut Object = msg_send![uuid, UUIDString];
        if uuid_string.is_null() {
            return None;
        }
        let tap_uid: CFString = CFString::wrap_under_get_rule(uuid_string as *const _);

        // A private aggregate device carries the tap stream; `tapautostart` makes
        // it start the tap for us.
        let sub_tap = CFDictionary::from_CFType_pairs(&[(
            CFString::new("uid"),
            tap_uid.as_CFType(),
        )]);
        let taps = CFArray::from_CFTypes(&[sub_tap.as_CFType()]);
        let dictionary = CFDictionary::from_CFType_pairs(&[
            (
                CFString::new("uid"),
                CFString::new(&format!(
                    "com.lanrhyme.micyou.aec.{}",
                    std::process::id()
                ))
                .as_CFType(),
            ),
            (
                CFString::new("name"),
                CFString::new("MicYou AEC Reference").as_CFType(),
            ),
            (CFString::new("private"), CFNumber::from(1).as_CFType()),
            (CFString::new("tapautostart"), CFNumber::from(1).as_CFType()),
            (CFString::new("taps"), taps.as_CFType()),
        ]);

        let mut aggregate: AudioObjectID = 0;
        let status = (api.create_aggregate_device)(
            dictionary.as_concrete_TypeRef() as *const c_void,
            &mut aggregate,
        );
        if status == 0 {
            Some(aggregate)
        } else {
            log::error!("[Loopback] AudioHardwareCreateAggregateDevice failed: {status}");
            None
        }
    }
}

unsafe fn release_description(description: *mut Object) {
    unsafe {
        let _: () = msg_send![description, release];
    }
}

fn cleanup(
    api: &CoreAudioApi,
    aggregate: AudioObjectID,
    io_proc: *mut c_void,
    tap: AudioObjectID,
    description: *mut Object,
    context: *mut TapContext,
) {
    unsafe {
        if !io_proc.is_null() {
            (api.stop_device)(aggregate, io_proc);
            (api.destroy_io_proc)(aggregate, io_proc);
        }
        (api.destroy_aggregate_device)(aggregate);
        (api.destroy_process_tap)(tap);
        release_description(description);
        // The IOProc is destroyed before its context is reclaimed.
        drop(Box::from_raw(context));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_resampler, fourcc, needs_resampler_rebuild, process_tap_available, resolve,
        SYMBOL_CREATE_PROCESS_TAP,
    };

    #[test]
    fn capability_probe_is_cached_and_consistent() {
        assert_eq!(process_tap_available(), process_tap_available());
    }

    #[test]
    fn missing_symbol_reports_unavailable() {
        // A symbol no Core Audio release exports must resolve to `None`
        // rather than panicking, which is what an older system looks like.
        assert!(resolve(b"MicYouDefinitelyNotACoreAudioSymbol\0").is_none());
    }

    #[test]
    fn framework_loads_and_exports_core_symbols() {
        // System frameworks live in the dyld shared cache, so their binary has no
        // on-disk path; the real check is that `dlopen`/`dlsym` resolve a symbol
        // Core Audio has always exported.
        assert!(resolve(b"AudioObjectGetPropertyData\0").is_some());
    }

    #[test]
    fn process_tap_symbol_name_is_nul_terminated() {
        let symbol =
            std::str::from_utf8(&SYMBOL_CREATE_PROCESS_TAP[..SYMBOL_CREATE_PROCESS_TAP.len() - 1])
                .expect("symbol must be valid UTF-8");
        assert_eq!(symbol, "AudioHardwareCreateProcessTap");
        assert_eq!(*SYMBOL_CREATE_PROCESS_TAP.last().unwrap(), 0);
    }

    #[test]
    fn rate_changes_are_followed_but_missing_formats_are_not() {
        assert!(needs_resampler_rebuild(44100, 48000));
        assert!(needs_resampler_rebuild(48000, 44100));
        // An unreadable format reports zero and must not trigger a rebuild.
        assert!(!needs_resampler_rebuild(44100, 0));
        assert!(!needs_resampler_rebuild(44100, 44100));
    }

    #[test]
    fn resampler_is_only_built_when_the_rate_differs() {
        assert!(build_resampler(crate::loopback::TARGET_RATE)
            .expect("the target rate needs no resampler")
            .is_none());
        assert!(build_resampler(44100)
            .expect("44100Hz needs a resampler")
            .is_some());
    }

    #[test]
    fn fourcc_matches_the_core_audio_property_codes() {
        assert_eq!(fourcc(b"glob"), 0x676C_6F62);
        assert_eq!(fourcc(b"id2p"), 0x6964_3270);
        assert_eq!(fourcc(b"tfmt"), 0x7466_6D74);
    }
}
