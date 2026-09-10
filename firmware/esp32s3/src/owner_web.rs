use core::{fmt::Write as _, sync::atomic::{AtomicBool, AtomicU16, Ordering}};

use embassy_net::{Stack, tcp::TcpSocket};
use embassy_time::{Duration, Instant, Timer};
use embedded_io_async::Write;
use heapless::{String, Vec};
use log::{info, warn};
use pattern_engine::{PatternSender, owner_limits};
use portable_atomic::AtomicU64;

use crate::owner_control::{
    OWNER_HEARTBEAT_INTERVAL_MS, OWNER_TIMEOUT_MS, heartbeat_fresh, process_line,
    set_wifi_local_limits_latched,
};
use crate::wifi_settings::{self, WifiCredentials, WifiFlash};
use crate::owner_settings;
use crate::owner_auth;

const BPM_MIN: u16 = 5;
const BPM_MAX: u16 = 300;
const BPM_TIMER_MAX_SECONDS: u32 = 60 * 60;

static BPM_ACTIVE: AtomicBool = AtomicBool::new(false);
static BPM_VALUE: AtomicU16 = AtomicU16::new(30);
static BPM_END_MS: AtomicU64 = AtomicU64::new(0);

fn apply_bpm_speed(patterns: &'static PatternSender, bpm: u16) {
    let input = patterns.input();
    let travel_mm = owner_limits::machine_travel_mm();
    let machine_max_speed = owner_limits::machine_max_speed_mm_s();
    let stroke_mm = input.stroke.clamp(0.0, 1.0) * travel_mm;
    // One BPM beat is one complete out-and-back cycle, so the commanded
    // path length per beat is twice the current effective stroke length.
    let required_mm_s = 2.0 * stroke_mm * bpm as f64 / 60.0;
    patterns.set_speed((required_mm_s / machine_max_speed).clamp(0.0, 1.0));
}

fn start_bpm(patterns: &'static PatternSender, bpm: u16, seconds: u32) -> Result<(), ()> {
    if !(BPM_MIN..=BPM_MAX).contains(&bpm) || seconds == 0 || seconds > BPM_TIMER_MAX_SECONDS {
        return Err(());
    }
    if owner_limits::estop_active() {
        return Err(());
    }

    // BPM is an owner-web run mode. Enable the existing owner envelope so
    // its configured limits remain authoritative during the timed run.
    owner_limits::set_master_enabled(true);
    let _ = owner_limits::activate_owner_session();
    set_wifi_local_limits_latched(true);
    patterns.reapply_owner_limits();

    BPM_VALUE.store(bpm, Ordering::Release);
    BPM_END_MS.store(Instant::now().as_millis().saturating_add(seconds as u64 * 1000), Ordering::Release);
    BPM_ACTIVE.store(true, Ordering::Release);
    apply_bpm_speed(patterns, bpm);
    patterns.play(0);
    info!("BPM run started: {} BPM for {} s", bpm, seconds);
    Ok(())
}

fn stop_bpm(patterns: &'static PatternSender) {
    BPM_ACTIVE.store(false, Ordering::Release);
    BPM_END_MS.store(0, Ordering::Release);
    patterns.stop();
    patterns.set_speed(0.0);
    info!("BPM run stopped");
}

#[embassy_executor::task]
pub async fn bpm_control_task(patterns: &'static PatternSender) {
    loop {
        if BPM_ACTIVE.load(Ordering::Acquire) {
            if owner_limits::estop_active() {
                BPM_ACTIVE.store(false, Ordering::Release);
                BPM_END_MS.store(0, Ordering::Release);
            } else {
                let now = Instant::now().as_millis();
                let end = BPM_END_MS.load(Ordering::Acquire);
                if end == 0 || now >= end {
                    stop_bpm(patterns);
                    info!("BPM timer complete");
                } else {
                    // Recalculate periodically so BLE/web stroke changes do not
                    // change the requested BPM. Owner speed limits still clamp it.
                    apply_bpm_speed(patterns, BPM_VALUE.load(Ordering::Acquire));
                }
            }
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}

const INDEX_HTML: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>OSSM Owner Control</title>
<style>
:root{color-scheme:light dark;font-family:system-ui,sans-serif}body{max-width:760px;margin:24px auto;padding:0 16px 40px}.card{border:1px solid #7777;border-radius:12px;padding:16px;margin:16px 0}.status{font-weight:700}.row{margin:18px 0}.row label{display:flex;justify-content:space-between;gap:12px}input[type=range],input[type=text],input[type=password],input[type=number]{width:100%;box-sizing:border-box;padding:9px}.buttons{display:flex;gap:10px;flex-wrap:wrap}button{padding:10px 14px;font-weight:650}.estop{font-size:1.15em;font-weight:800}small{opacity:.75}.field{margin:12px 0}.field label{display:block;margin-bottom:5px;font-weight:650}
</style>
</head>
<body>
<h1>OSSM Owner Control</h1>
<p>Local owner limits. BLE/XToys remains connected while this page is used.</p>
<div class="card">
 <div id="status" class="status">Connecting...</div>
 <div id="mode">Loading state...</div>
 <small>No periodic Wi-Fi heartbeat. Network traffic occurs only when loading the page or changing a setting.</small>
 <div class="buttons" style="margin-top:14px">
  <button id="masterOn">Master ON</button><button id="masterOff">Master OFF / stock</button>
  <button id="enable">Owner session ON</button><button id="fallback">End page session</button><button id="refreshBtn">Refresh</button>
 </div>
</div>
<div class="card">
 <h2>Machine configuration</h2>
 <div class="field"><label for="machineMaxSpeed">Machine max speed (mm/s)</label><input id="machineMaxSpeed" type="number" min="1" max="900" step="1" value="600"></div>
 <div class="field"><label for="machineLength">Machine max length (mm)</label><input id="machineLength" type="number" min="1" max="400" step="1" value="180"></div>
 <small>Hard safety ceiling for all modes. Saving reboots the controller so the low-level motion limits and all 0-100% scaling use the same values.</small>
 <div class="buttons" style="margin-top:14px"><button id="machineSave">Save machine settings & reboot</button></div>
 <p id="machineResult"></p>
</div>
<div class="card">
 <h2>Owner limits</h2>
 <div class="row"><label><span>Maximum speed</span><strong><span id="maxSpeedText">150</span> mm/s</strong></label><input id="maxSpeed" type="range" min="0" max="900" step="1" value="150"></div>
 <div class="row"><label><span>Minimum stroke</span><strong><span id="minStrokeText">0</span> mm</strong></label><input id="minStroke" type="range" min="0" max="400" step="1" value="0"></div>
 <div class="row"><label><span>Maximum stroke</span><strong><span id="maxStrokeText">100</span> mm</strong></label><input id="maxStroke" type="range" min="0" max="400" step="1" value="100"></div>
 <div class="row"><label><span>Minimum depth</span><strong><span id="minDepthText">0</span> mm</strong></label><input id="minDepth" type="range" min="0" max="400" step="1" value="0"></div>
 <div class="row"><label><span>Maximum depth</span><strong><span id="maxDepthText">160</span> mm</strong></label><input id="maxDepth" type="range" min="0" max="400" step="1" value="160"></div>
 <div id="fallbackInfo"></div>
</div>
<div class="card">
 <h2>BPM timed run</h2>
 <div class="row"><label><span>Beat rate</span><strong><span id="bpmText">30</span> BPM</strong></label><input id="bpm" type="range" min="5" max="300" step="1" value="30"></div>
 <div class="field"><label for="bpmMinutes">Run timer (minutes)</label><input id="bpmMinutes" type="number" min="1" max="60" step="1" value="5"></div>
 <small>1 BPM beat = one complete out-and-back stroke cycle. Required speed is calculated from the current stroke and clamped by the owner maximum-speed limit.</small>
 <div class="buttons" style="margin-top:14px"><button id="bpmStart">Start BPM</button><button id="bpmStop">Stop</button></div>
 <p id="bpmState">Stopped</p>
</div>
<div class="card"><div class="buttons"><button id="estop" class="estop">E-STOP</button><button id="resetEstop">Reset E-stop</button></div><div id="estopState"></div></div>

<div class="card">
 <h2>Wi-Fi settings</h2>
 <p id="wifiCurrent">Loading saved Wi-Fi settings...</p>
 <div class="field"><label for="wifiSsid">SSID</label><input id="wifiSsid" type="text" maxlength="32" autocomplete="off"></div>
 <div class="field"><label for="wifiPassword">New password</label><input id="wifiPassword" type="password" maxlength="64" autocomplete="new-password"></div>
 <small>Leave password blank to keep the existing saved password. The stored password is never displayed.</small>
 <div class="buttons" style="margin-top:14px"><button id="wifiSave">Save Wi-Fi & reboot</button><button id="wifiClear">Clear saved Wi-Fi & reboot</button></div>
 <p id="wifiResult"></p>
</div>
<script>
const $=id=>document.getElementById(id);const controls=["maxSpeed","minStroke","maxStroke","minDepth","maxDepth"];let init=false;
function setControl(id,v){$(id).value=Math.round(v);$(id+"Text").textContent=Math.round(v)}
function applyState(s){$("machineMaxSpeed").value=Math.round(s.machine_max_speed);$("machineLength").value=Math.round(s.machine_max_travel);$("maxSpeed").max=Math.round(s.machine_max_speed);for(const id of ["minStroke","maxStroke","minDepth","maxDepth"])$(id).max=Math.round(s.machine_max_travel);init=true;setControl("maxSpeed",s.max_speed);setControl("minStroke",s.min_stroke);setControl("maxStroke",s.max_stroke);setControl("minDepth",s.min_depth);setControl("maxDepth",s.max_depth);$("mode").textContent=!s.master_enabled?"STOCK - MASTER OFF":(s.owner_active?"OWNER LIMITS ACTIVE - PAGE AUTHENTICATED":"OWNER LIMITS ACTIVE - SAVED SETTINGS");$("estopState").textContent=s.estop_active?"E-STOP LATCHED":"E-stop reset";$("fallbackInfo").textContent=`Saved owner limits remain active even when this page disconnects.`}
async function post(path){const r=await fetch(path,{method:"POST",cache:"no-store"});if(!r.ok)throw new Error(await r.text()||path+" "+r.status);return r}
async function refresh(){try{const r=await fetch('/api/state',{cache:'no-store'});if(r.ok){applyState(await r.json());$("status").textContent="WIFI CONNECTED"}}catch(_){$("status").textContent="CONNECTION LOST"}}
async function refreshWifi(){try{const r=await fetch('/api/wifi',{cache:'no-store'});if(r.ok){const ssid=await r.text();$("wifiCurrent").textContent=ssid?`Saved SSID: ${ssid}`:"No saved Wi-Fi override (factory/build setting may be active).";if(ssid&&!$("wifiSsid").value)$("wifiSsid").value=ssid}}catch(_){}}
let bpmCountdown=null;
function showBpmState(s){clearInterval(bpmCountdown);if(!s.active){$("bpmState").textContent="Stopped";return}let remaining=Math.ceil(s.remaining_ms/1000);const render=()=>{$("bpmState").textContent=`Running ${s.bpm} BPM - ${Math.floor(remaining/60)}:${String(remaining%60).padStart(2,'0')} remaining`;if(remaining>0)remaining--;else{clearInterval(bpmCountdown);$("bpmState").textContent="Timer complete - stopped"}};render();bpmCountdown=setInterval(render,1000)}

async function refreshBpm(){try{const r=await fetch('/api/bpm',{cache:'no-store'});if(r.ok)showBpmState(await r.json())}catch(_){}}
async function sendLimits(){let a=+$("minStroke").value,b=+$("maxStroke").value,c=+$("minDepth").value,d=+$("maxDepth").value;if(a>b){b=a;setControl('maxStroke',b)}if(c>d){d=c;setControl('maxDepth',d)}const q=new URLSearchParams({max_speed:$("maxSpeed").value,min_stroke:a,max_stroke:b,min_depth:c,max_depth:d});await post('/api/limits?'+q);await refresh()}
controls.forEach(id=>{$(id).addEventListener('input',()=>$(id+'Text').textContent=$(id).value);$(id).addEventListener('change',sendLimits)});
$("machineSave").onclick=async()=>{const speed=+$('machineMaxSpeed').value,length=+$('machineLength').value;if(speed<1||speed>900||length<1||length>400){$("machineResult").textContent="Use speed 1-900 mm/s and length 1-400 mm.";return}if(!confirm(`Save machine limits ${speed} mm/s and ${length} mm, then reboot?`))return;$("machineResult").textContent="Saving...";try{await post(`/api/machine?max_speed=${speed}&length=${length}`);$("machineResult").textContent="Saved. Device is rebooting..."}catch(e){$("machineResult").textContent="Save failed: "+e.message}};
$("masterOn").onclick=async()=>{await post('/api/master/enable');await refresh()};$("masterOff").onclick=async()=>{await post('/api/master/disable');await refresh()};$("enable").onclick=async()=>{await post('/api/enable');await refresh()};$("fallback").onclick=async()=>{await post('/api/disable');await refresh()};$("refreshBtn").onclick=refresh;$("estop").onclick=async()=>{await post('/api/estop');await refresh()};$("resetEstop").onclick=async()=>{await post('/api/estop/reset');await refresh()};
$("bpm").addEventListener('input',()=>$("bpmText").textContent=$("bpm").value);
$("bpmStart").onclick=async()=>{const bpm=+$('bpm').value,minutes=+$('bpmMinutes').value;if(minutes<1||minutes>60)return;try{await post(`/api/bpm/start?bpm=${bpm}&seconds=${Math.round(minutes*60)}`);await refresh();await refreshBpm()}catch(e){$("bpmState").textContent="Start failed: "+e.message}};
$("bpmStop").onclick=async()=>{try{await post('/api/bpm/stop');await refreshBpm()}catch(e){$("bpmState").textContent="Stop failed: "+e.message}};
$("wifiSave").onclick=async()=>{const ssid=$("wifiSsid").value.trim(),password=$("wifiPassword").value;if(!ssid){$("wifiResult").textContent="Enter an SSID.";return}$("wifiResult").textContent="Saving...";try{const q=new URLSearchParams({ssid,password,keep_password:password?'0':'1'});await post('/api/wifi/save?'+q);$("wifiResult").textContent="Saved. Device is rebooting..."}catch(e){$("wifiResult").textContent="Save failed: "+e.message}};
$("wifiClear").onclick=async()=>{if(!confirm('Clear saved Wi-Fi settings and reboot?'))return;$("wifiResult").textContent="Clearing...";try{await post('/api/wifi/clear');$("wifiResult").textContent="Cleared. Device is rebooting..."}catch(e){$("wifiResult").textContent="Clear failed: "+e.message}};
(async()=>{await refresh();await refreshWifi();await refreshBpm()})();
</script></body></html>"#;

const AUTH_SETUP_HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>OSSM Owner Password</title>
<style>:root{color-scheme:light dark;font-family:system-ui,sans-serif}body{max-width:520px;margin:32px auto;padding:0 18px}.card{border:1px solid #7777;border-radius:14px;padding:20px}.field{margin:16px 0}.field label{display:block;font-weight:700;margin-bottom:6px}input{width:100%;box-sizing:border-box;padding:11px;font-size:1em}button{padding:11px 16px;font-weight:700}small{opacity:.75}</style></head>
<body><h1>Create owner password</h1><div class="card"><p>This firmware requires an owner password before configuration changes are accepted.</p>
<div class="field"><label for="password">Owner password / PIN</label><input id="password" type="password" minlength="4" maxlength="64" autocomplete="new-password"></div>
<small>Minimum 4 characters. This protects owner-page changes from other devices on the local network.</small><p><button id="save">Set password</button></p><p id="result"></p></div>
<script>const $=id=>document.getElementById(id);$("save").onclick=async()=>{const password=$("password").value;if(password.length<4){$("result").textContent="Use at least 4 characters.";return}$("result").textContent="Saving...";try{const q=new URLSearchParams({password});const r=await fetch('/api/auth/setup?'+q,{method:'POST',cache:'no-store'});if(!r.ok)throw new Error(await r.text());$("result").textContent="Saved. Reloading...";setTimeout(()=>location.reload(),700)}catch(e){$("result").textContent="Save failed: "+e.message}}</script></body></html>"#;

const SETUP_HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>OSSM Wi-Fi Setup</title>
<style>:root{color-scheme:light dark;font-family:system-ui,sans-serif}body{max-width:520px;margin:32px auto;padding:0 18px}.card{border:1px solid #7777;border-radius:14px;padding:20px}.field{margin:16px 0}.field label{display:block;font-weight:700;margin-bottom:6px}input{width:100%;box-sizing:border-box;padding:11px;font-size:1em}button{padding:11px 16px;font-weight:700}small{opacity:.75}</style></head>
<body><h1>OSSM Wi-Fi Setup</h1><div class="card"><p>Enter the Wi-Fi network this OSSM should use. Settings are stored on the controller.</p>
<div class="field"><label for="ssid">Wi-Fi name (SSID)</label><input id="ssid" maxlength="32" autocomplete="off"></div>
<div class="field"><label for="password">Wi-Fi password</label><input id="password" type="password" maxlength="64" autocomplete="new-password"></div>
<div class="field"><label for="ownerPassword">Owner password / PIN</label><input id="ownerPassword" type="password" minlength="4" maxlength="64" autocomplete="new-password"></div>
<small>On first setup, create an owner password (minimum 4 characters). On recovery, your existing owner password is requested by the browser before changes are accepted.</small><p><button id="save">Save & reboot</button></p><p id="result"></p></div>
<script>const $=id=>document.getElementById(id);$("save").onclick=async()=>{const ssid=$("ssid").value.trim(),password=$("password").value;if(!ssid){$("result").textContent="Enter a Wi-Fi name.";return}$("result").textContent="Saving...";try{const q=new URLSearchParams({ssid,password,keep_password:'0',owner_password:$("ownerPassword").value});const r=await fetch('/api/wifi/save?'+q,{method:'POST',cache:'no-store'});if(!r.ok)throw new Error(await r.text());$("result").textContent="Saved. OSSM is rebooting and will join your Wi-Fi network."}catch(e){$("result").textContent="Save failed: "+e.message}}</script></body></html>"#;

fn query_value<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    let query_start = request.find('?')?;
    let query = &request[query_start + 1..];
    let query_end = query.find(' ').unwrap_or(query.len());
    for pair in query[..query_end].split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == name {
            return parts.next();
        }
    }
    None
}

fn query_f64(request: &str, name: &str) -> Option<f64> {
    query_value(request, name)?.parse().ok()
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn decode_query<const N: usize>(raw: &str) -> Option<String<N>> {
    let mut out = String::<N>::new();
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = match bytes[i] {
            b'+' => b' ',
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_value(bytes[i + 1])?;
                let lo = hex_value(bytes[i + 2])?;
                i += 2;
                (hi << 4) | lo
            }
            b'%' => return None,
            other => other,
        };
        out.push(b as char).ok()?;
        i += 1;
    }
    Some(out)
}


fn b64_value(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn basic_password(request: &str) -> Option<String<64>> {
    let auth = request.lines().find_map(|line| line.strip_prefix("Authorization: Basic "))?;
    let raw = auth.trim().as_bytes();
    let mut decoded = Vec::<u8, 96>::new();
    let mut i = 0usize;
    while i < raw.len() {
        let mut vals = [0u8; 4];
        let mut pad = 0usize;
        for j in 0..4 {
            let b = *raw.get(i + j)?;
            if b == b'=' { vals[j] = 0; pad += 1; }
            else { vals[j] = b64_value(b)?; }
        }
        decoded.push((vals[0] << 2) | (vals[1] >> 4)).ok()?;
        if pad < 2 { decoded.push((vals[1] << 4) | (vals[2] >> 2)).ok()?; }
        if pad < 1 { decoded.push((vals[2] << 6) | vals[3]).ok()?; }
        i += 4;
    }
    let colon = decoded.iter().position(|b| *b == b':')?;
    let pass = core::str::from_utf8(&decoded[colon + 1..]).ok()?;
    let mut out = String::<64>::new();
    out.push_str(pass).ok()?;
    Some(out)
}

async fn request_authorized(request: &str, flash: &'static WifiFlash) -> bool {
    if !owner_auth::configured() { return false; }
    let Some(password) = basic_password(request) else { return false; };
    owner_auth::verify(flash, password.as_str()).await
}

async fn send_auth_required(socket: &mut TcpSocket<'_>) {
    let body = b"Owner authentication required";
    let mut header = String::<320>::new();
    let _ = write!(header,
        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"OSSM Owner\"\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len());
    let _ = socket.write_all(header.as_bytes()).await;
    let _ = socket.write_all(body).await;
    let _ = socket.flush().await;
}

async fn send_response(socket: &mut TcpSocket<'_>, status: &str, content_type: &str, body: &[u8]) {
    let mut header = String::<256>::new();
    let _ = write!(
        header,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let _ = socket.write_all(header.as_bytes()).await;
    let _ = socket.write_all(body).await;
    let _ = socket.flush().await;
}

async fn send_state(socket: &mut TcpSocket<'_>) {
    let owner = owner_limits::owner_limits();
    let fb = owner_limits::fallback_limits();
    let mut body = String::<640>::new();
    let _ = write!(
        body,
        concat!(
            "{{\"master_enabled\":{},\"owner_active\":{},\"estop_active\":{},\"heartbeat_fresh\":{},",
            "\"max_speed\":{:.1},\"min_stroke\":{:.1},\"max_stroke\":{:.1},\"min_depth\":{:.1},\"max_depth\":{:.1},",
            "\"fallback_max_speed\":{:.1},\"fallback_min_stroke\":{:.1},\"fallback_max_stroke\":{:.1},\"fallback_min_depth\":{:.1},\"fallback_max_depth\":{:.1},",
            "\"machine_max_speed\":{:.1},\"machine_max_travel\":{:.1},\"heartbeat_ms\":{},\"timeout_ms\":{}}}"
        ),
        owner_limits::master_enabled(), owner_limits::owner_session_active(), owner_limits::estop_active(), heartbeat_fresh(),
        owner.max_speed, owner.min_stroke, owner.max_stroke, owner.min_depth, owner.max_depth,
        fb.max_speed, fb.min_stroke, fb.max_stroke, fb.min_depth, fb.max_depth,
        owner_limits::machine_max_speed_mm_s(), owner_limits::machine_travel_mm(), OWNER_HEARTBEAT_INTERVAL_MS, OWNER_TIMEOUT_MS
    );
    send_response(socket, "200 OK", "application/json", body.as_bytes()).await;
}

async fn send_bpm_state(socket: &mut TcpSocket<'_>) {
    let active = BPM_ACTIVE.load(Ordering::Acquire);
    let bpm = BPM_VALUE.load(Ordering::Acquire);
    let end = BPM_END_MS.load(Ordering::Acquire);
    let now = Instant::now().as_millis();
    let remaining_ms = if active { end.saturating_sub(now) } else { 0 };
    let input = owner_limits::clamp_input(pattern_engine::owner_limits::requested_input());
    let stroke_mm = input.stroke * owner_limits::machine_travel_mm();
    let effective_speed = input.velocity * owner_limits::machine_max_speed_mm_s();
    let actual_bpm = if stroke_mm > 0.001 { effective_speed * 60.0 / (2.0 * stroke_mm) } else { 0.0 };
    let mut body = String::<256>::new();
    let _ = write!(body, "{{\"active\":{},\"bpm\":{},\"remaining_ms\":{},\"actual_bpm\":{:.1}}}", active, bpm, remaining_ms, actual_bpm);
    send_response(socket, "200 OK", "application/json", body.as_bytes()).await;
}

async fn send_wifi_ssid(socket: &mut TcpSocket<'_>, flash: &'static WifiFlash) {
    if let Some(c) = wifi_settings::load(flash).await {
        send_response(socket, "200 OK", "text/plain; charset=utf-8", c.ssid.as_bytes()).await;
    } else {
        send_response(socket, "200 OK", "text/plain; charset=utf-8", b"").await;
    }
}

async fn handle_wifi_save(socket: &mut TcpSocket<'_>, request: &str, flash: &'static WifiFlash) -> bool {
    if owner_auth::configured() {
        if !request_authorized(request, flash).await {
            send_auth_required(socket).await;
            return false;
        }
    } else {
        let owner_password = match query_value(request, "owner_password").and_then(decode_query::<64>) {
            Some(v) if v.len() >= 4 => v,
            _ => {
                send_response(socket, "400 Bad Request", "text/plain", b"Create an owner password of at least 4 characters").await;
                return false;
            }
        };
        if owner_auth::save_password(flash, owner_password.as_str()).await.is_err() {
            send_response(socket, "500 Internal Server Error", "text/plain", b"Owner password save failed").await;
            return false;
        }
    }

    let ssid = match query_value(request, "ssid").and_then(decode_query::<32>) {
        Some(v) if !v.is_empty() => v,
        _ => {
            send_response(socket, "400 Bad Request", "text/plain", b"Invalid SSID").await;
            return false;
        }
    };
    let mut password = match query_value(request, "password").and_then(decode_query::<64>) {
        Some(v) => v,
        None => {
            send_response(socket, "400 Bad Request", "text/plain", b"Invalid password").await;
            return false;
        }
    };

    let keep_password = query_value(request, "keep_password") == Some("1");
    if keep_password && password.is_empty() {
        if let Some(existing) = wifi_settings::load(flash).await {
            password = existing.password;
        }
    }

    let credentials = match WifiCredentials::new(ssid.as_str(), password.as_str()) {
        Some(v) => v,
        None => {
            send_response(socket, "400 Bad Request", "text/plain", b"Wi-Fi settings out of range").await;
            return false;
        }
    };
    match wifi_settings::save(flash, &credentials).await {
        Ok(()) => {
            send_response(socket, "200 OK", "text/plain", b"Saved; rebooting").await;
            true
        }
        Err(()) => {
            send_response(socket, "500 Internal Server Error", "text/plain", b"Flash write failed").await;
            false
        }
    }
}

async fn maybe_reboot(reboot: bool) -> ! {
    if reboot {
        Timer::after(Duration::from_millis(400)).await;
        esp_hal::system::software_reset();
    }
    loop {
        Timer::after(Duration::from_secs(3600)).await;
    }
}

#[embassy_executor::task]
pub async fn wifi_setup_web_task(stack: Stack<'static>, flash: &'static WifiFlash) {
    stack.wait_config_up().await;
    info!("OSSM setup page: http://192.168.4.1/");
    loop {
        let mut rx = [0u8; 2048];
        let mut tx = [0u8; 2048];
        let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
        socket.set_timeout(Some(Duration::from_secs(5)));
        if socket.accept(80).await.is_err() { continue; }
        let mut req = [0u8; 1024];
        let n = match socket.read(&mut req).await { Ok(n) if n > 0 => n, _ => continue };
        let request = core::str::from_utf8(&req[..n]).unwrap_or("");
        let reboot = if request.starts_with("POST /api/wifi/save?") {
            handle_wifi_save(&mut socket, request, flash).await
        } else if request.starts_with("GET /favicon.ico ") {
            send_response(&mut socket, "204 No Content", "text/plain", b"").await;
            false
        } else if owner_auth::configured() && !request_authorized(request, flash).await {
            send_auth_required(&mut socket).await;
            false
        } else {
            // Serve setup/recovery UI for root and common captive-portal probe paths.
            send_response(&mut socket, "200 OK", "text/html; charset=utf-8", SETUP_HTML.as_bytes()).await;
            false
        };
        socket.close();
        if reboot { maybe_reboot(true).await; }
    }
}

#[embassy_executor::task]
pub async fn owner_web_task(
    stack: Stack<'static>,
    patterns: &'static PatternSender,
    flash: &'static WifiFlash,
) {
    stack.wait_config_up().await;
    if let Some(c) = stack.config_v4() {
        info!("Owner Wi-Fi page: http://{}/", c.address.address());
    }
    loop {
        let mut rx = [0u8; 2048];
        let mut tx = [0u8; 2048];
        let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
        socket.set_timeout(Some(Duration::from_secs(5)));
        if socket.accept(80).await.is_err() { continue; }
        let mut req = [0u8; 1024];
        let n = match socket.read(&mut req).await { Ok(n) if n > 0 => n, _ => continue };
        let request = core::str::from_utf8(&req[..n]).unwrap_or("");
        let mut reboot = false;

        // E-stop remains intentionally available without authentication.
        if request.starts_with("POST /api/estop/reset ") {
            process_line("@OWNER:ESTOP:RESET", patterns);
            send_response(&mut socket,"200 OK","text/plain",b"OK").await;
        }
        else if request.starts_with("POST /api/estop ") {
            process_line("@OWNER:ESTOP", patterns);
            send_response(&mut socket,"200 OK","text/plain",b"OK").await;
        }
        else if request.starts_with("POST /api/auth/setup?") {
            if owner_auth::configured() {
                send_response(&mut socket,"409 Conflict","text/plain",b"Owner password already configured").await;
            } else {
                let password = query_value(request, "password").and_then(decode_query::<64>);
                match password {
                    Some(password) if password.len() >= 4 => match owner_auth::save_password(flash, password.as_str()).await {
                        Ok(()) => send_response(&mut socket,"200 OK","text/plain",b"OK").await,
                        Err(()) => send_response(&mut socket,"500 Internal Server Error","text/plain",b"Password save failed").await,
                    },
                    _ => send_response(&mut socket,"400 Bad Request","text/plain",b"Password must be 4-64 characters").await,
                }
            }
        }
        else if request.starts_with("GET /favicon.ico ") {
            send_response(&mut socket,"204 No Content","text/plain",b"").await;
        }
        else if !owner_auth::configured() {
            if request.starts_with("GET / ") {
                send_response(&mut socket,"200 OK","text/html; charset=utf-8",AUTH_SETUP_HTML.as_bytes()).await;
            } else {
                send_response(&mut socket,"403 Forbidden","text/plain",b"Create owner password first").await;
            }
        }
        else if !request_authorized(request, flash).await {
            send_auth_required(&mut socket).await;
        }
        else if request.starts_with("POST /api/heartbeat ") { process_line("@OWNER:HB", patterns); send_response(&mut socket,"200 OK","text/plain",b"OK").await }
        else if request.starts_with("POST /api/master/enable ") {
            process_line("@OWNER:MASTER:ENABLE", patterns);
            match owner_settings::save_current(flash).await {
                Ok(()) => send_response(&mut socket,"200 OK","text/plain",b"OK").await,
                Err(()) => send_response(&mut socket,"500 Internal Server Error","text/plain",b"Limits applied but flash save failed").await,
            }
        }
        else if request.starts_with("POST /api/master/disable ") {
            process_line("@OWNER:MASTER:DISABLE", patterns);
            match owner_settings::save_current(flash).await {
                Ok(()) => send_response(&mut socket,"200 OK","text/plain",b"OK").await,
                Err(()) => send_response(&mut socket,"500 Internal Server Error","text/plain",b"Limits applied but flash save failed").await,
            }
        }
        else if request.starts_with("POST /api/enable ") { process_line("@OWNER:ENABLE", patterns); send_response(&mut socket,"200 OK","text/plain",b"OK").await }
        else if request.starts_with("POST /api/disable ") { process_line("@OWNER:DISABLE", patterns); send_response(&mut socket,"200 OK","text/plain",b"OK").await }
        else if request.starts_with("POST /api/machine?") {
            let vals = (query_f64(request,"max_speed"), query_f64(request,"length"));
            if let (Some(max_speed), Some(length)) = vals {
                // Stop motion before persisting a machine-envelope change. The
                // new values take effect only after the immediate reboot.
                patterns.stop();
                match owner_settings::save_machine(flash, max_speed, length).await {
                    Ok(()) => {
                        send_response(&mut socket,"200 OK","text/plain",b"Saved; rebooting").await;
                        reboot = true;
                    }
                    Err(()) => send_response(&mut socket,"400 Bad Request","text/plain",b"Machine limits must be speed 1-900 mm/s and length 1-400 mm").await,
                }
            } else { send_response(&mut socket,"400 Bad Request","text/plain",b"Bad machine limits").await }
        }
        else if request.starts_with("POST /api/limits?") {
            let vals = (query_f64(request,"max_speed"),query_f64(request,"min_stroke"),query_f64(request,"max_stroke"),query_f64(request,"min_depth"),query_f64(request,"max_depth"));
            if let (Some(a),Some(b),Some(c),Some(d),Some(e)) = vals {
                process_line("@OWNER:MASTER:ENABLE", patterns);
                process_line("@OWNER:ENABLE", patterns);
                set_wifi_local_limits_latched(true);
                let mut line = String::<160>::new();
                let _ = write!(line,"@OWNER:LIMITS:{}:{}:{}:{}:{}",a,b,c,d,e);
                process_line(&line, patterns);
                match owner_settings::save_current(flash).await {
                    Ok(()) => send_response(&mut socket,"200 OK","text/plain",b"OK").await,
                    Err(()) => send_response(&mut socket,"500 Internal Server Error","text/plain",b"Limits applied but flash save failed").await,
                }
            } else { send_response(&mut socket,"400 Bad Request","text/plain",b"Bad limits").await }
        }
        else if request.starts_with("POST /api/bpm/start?") {
            let bpm = query_value(request, "bpm").and_then(|v| v.parse::<u16>().ok());
            let seconds = query_value(request, "seconds").and_then(|v| v.parse::<u32>().ok());
            if let (Some(bpm), Some(seconds)) = (bpm, seconds) {
                match start_bpm(patterns, bpm, seconds) {
                    Ok(()) => send_response(&mut socket,"200 OK","text/plain",b"OK").await,
                    Err(()) => send_response(&mut socket,"400 Bad Request","text/plain",b"Invalid BPM/timer or E-stop active").await,
                }
            } else { send_response(&mut socket,"400 Bad Request","text/plain",b"Bad BPM request").await }
        }
        else if request.starts_with("POST /api/bpm/stop ") { stop_bpm(patterns); send_response(&mut socket,"200 OK","text/plain",b"OK").await }
        else if request.starts_with("GET /api/bpm ") { send_bpm_state(&mut socket).await }
        else if request.starts_with("POST /api/wifi/save?") { reboot = handle_wifi_save(&mut socket, request, flash).await; }
        else if request.starts_with("POST /api/wifi/clear ") {
            match wifi_settings::clear(flash).await {
                Ok(()) => { send_response(&mut socket,"200 OK","text/plain",b"Cleared; rebooting").await; reboot = true; }
                Err(()) => send_response(&mut socket,"500 Internal Server Error","text/plain",b"Flash erase failed").await,
            }
        }
        else if request.starts_with("GET /api/wifi ") { send_wifi_ssid(&mut socket, flash).await }
        else if request.starts_with("GET /api/state ") { send_state(&mut socket).await }
        else if request.starts_with("GET / ") { send_response(&mut socket,"200 OK","text/html; charset=utf-8",INDEX_HTML.as_bytes()).await }
        else { warn!("Unknown owner web request"); send_response(&mut socket,"404 Not Found","text/plain",b"Not Found").await }

        socket.close();
        if reboot { maybe_reboot(true).await; }
    }
}
