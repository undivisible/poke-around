// bridge/node_modules/poke/dist/index.mjs
import { createRequire } from "module";
import Zt from "node:fs";
import Xs from "node:fs";
import er from "node:process";
import Js from "node:os";
import Qs from "node:fs";
import tr from "node:process";
import rr, { constants as en } from "node:fs/promises";
import { promisify as sn } from "node:util";
import nn from "node:process";
import { execFile as on } from "node:child_process";
import ln from "node:process";
import { promisify as cn } from "node:util";
import { execFile as hn, execFileSync as Qo } from "node:child_process";
import { promisify as un } from "node:util";
import { execFile as dn } from "node:child_process";
import { promisify as gn } from "node:util";
import ht from "node:process";
import { execFile as yn } from "node:child_process";
import mr from "node:process";
import { Buffer as gr } from "node:buffer";
import yr from "node:path";
import { fileURLToPath as vn } from "node:url";
import { promisify as Sn } from "node:util";
import wr from "node:child_process";
import xn, { constants as bn } from "node:fs/promises";
import ye from "fs";
import zs from "os";
import Je from "path";
import Fn from "node:http";
var require2 = createRequire(import.meta.url);
var Ms = Object.create;
var Ze = Object.defineProperty;
var Ds = Object.getOwnPropertyDescriptor;
var $s = Object.getOwnPropertyNames;
var js = Object.getPrototypeOf;
var qs = Object.prototype.hasOwnProperty;
var v = ((r) => typeof require2 < "u" ? require2 : typeof Proxy < "u" ? new Proxy(r, { get: (e, t) => (typeof require2 < "u" ? require2 : e)[t] }) : r)(function(r) {
  if (typeof require2 < "u")
    return require2.apply(this, arguments);
  throw Error('Dynamic require of "' + r + '" is not supported');
});
var P = (r, e) => () => (r && (e = r(r = 0)), e);
var b = (r, e) => () => (e || r((e = { exports: {} }).exports, e), e.exports);
var Gs = (r, e) => {
  for (var t in e)
    Ze(r, t, { get: e[t], enumerable: true });
};
var Hs = (r, e, t, s) => {
  if (e && typeof e == "object" || typeof e == "function")
    for (let n of $s(e))
      !qs.call(r, n) && n !== t && Ze(r, n, { get: () => e[n], enumerable: !(s = Ds(e, n)) || s.enumerable });
  return r;
};
var ge = (r, e, t) => (t = r != null ? Ms(js(r)) : {}, Hs(e || !r || !r.__esModule ? Ze(t, "default", { value: r, enumerable: true }) : t, r));
function Ys() {
  try {
    return Zt.statSync("/.dockerenv"), true;
  } catch {
    return false;
  }
}
function Ks() {
  try {
    return Zt.readFileSync("/proc/self/cgroup", "utf8").includes("docker");
  } catch {
    return false;
  }
}
function rt() {
  return tt === undefined && (tt = Ys() || Ks()), tt;
}
var tt;
var Jt = P(() => {});
function re() {
  return st === undefined && (st = Zs() || rt()), st;
}
var st;
var Zs;
var nt = P(() => {
  Jt();
  Zs = () => {
    try {
      return Xs.statSync("/run/.containerenv"), true;
    } catch {
      return false;
    }
  };
});
var Qt;
var F;
var it = P(() => {
  nt();
  Qt = () => {
    if (er.platform !== "linux")
      return false;
    if (Js.release().toLowerCase().includes("microsoft"))
      return !re();
    try {
      return Qs.readFileSync("/proc/version", "utf8").toLowerCase().includes("microsoft") ? !re() : false;
    } catch {
      return false;
    }
  }, F = er.env.__IS_WSL_TEST__ ? Qt : Qt();
});
var tn;
var rn;
var ot;
var sr = P(() => {
  it();
  it();
  tn = (() => {
    let r = "/mnt/", e;
    return async function() {
      if (e)
        return e;
      let t = "/etc/wsl.conf", s = false;
      try {
        await rr.access(t, en.F_OK), s = true;
      } catch {}
      if (!s)
        return r;
      let n = await rr.readFile(t, { encoding: "utf8" }), i = /(?<!#.*)root\s*=\s*(?<mountPoint>.*)/g.exec(n);
      return i ? (e = i.groups.mountPoint.trim(), e = e.endsWith("/") ? e : `${e}/`, e) : r;
    };
  })(), rn = async () => `${await tn()}c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe`, ot = async () => F ? rn() : `${tr.env.SYSTEMROOT || tr.env.windir || String.raw`C:\Windows`}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe`;
});
function M(r, e, t) {
  let s = (n) => Object.defineProperty(r, e, { value: n, enumerable: true, writable: true });
  return Object.defineProperty(r, e, { configurable: true, enumerable: true, get() {
    let n = t();
    return s(n), n;
  }, set(n) {
    s(n);
  } }), r;
}
var nr = P(() => {});
async function at() {
  if (nn.platform !== "darwin")
    throw new Error("macOS only");
  let { stdout: r } = await an("defaults", ["read", "com.apple.LaunchServices/com.apple.launchservices.secure", "LSHandlers"]);
  return /LSHandlerRoleAll = "(?!-)(?<id>[^"]+?)";\s+?LSHandlerURLScheme = (?:http|https);/.exec(r)?.groups.id ?? "com.apple.Safari";
}
var an;
var ir = P(() => {
  an = sn(on);
});
async function or(r, { humanReadableOutput: e = true, signal: t } = {}) {
  if (ln.platform !== "darwin")
    throw new Error("macOS only");
  let s = e ? [] : ["-ss"], n = {};
  t && (n.signal = t);
  let { stdout: i } = await fn("osascript", ["-e", r, s], n);
  return i.trim();
}
var fn;
var ar = P(() => {
  fn = cn(hn);
});
async function lt(r) {
  return or(`tell application "Finder" to set app_path to application file id "${r}" as string
tell application "System Events" to get value of property list item "CFBundleName" of property list file (app_path & ":Contents:Info.plist")`);
}
var lr = P(() => {
  ar();
});
async function ct(r = pn) {
  let { stdout: e } = await r("reg", ["QUERY", " HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\http\\UserChoice", "/v", "ProgId"]), t = /ProgId\s*REG_SZ\s*(?<id>\S+)/.exec(e);
  if (!t)
    throw new Oe(`Cannot find Windows browser in stdout: ${JSON.stringify(e)}`);
  let { id: s } = t.groups, n = mn[s];
  if (!n)
    throw new Oe(`Unknown browser ID: ${s}`);
  return n;
}
var pn;
var mn;
var Oe;
var cr = P(() => {
  pn = un(dn), mn = { AppXq0fevzme2pys62n3e0fbqa7peapykr8v: { name: "Edge", id: "com.microsoft.edge.old" }, MSEdgeDHTML: { name: "Edge", id: "com.microsoft.edge" }, MSEdgeHTM: { name: "Edge", id: "com.microsoft.edge" }, "IE.HTTP": { name: "Internet Explorer", id: "com.microsoft.ie" }, FirefoxURL: { name: "Firefox", id: "org.mozilla.firefox" }, ChromeHTML: { name: "Chrome", id: "com.google.chrome" }, BraveHTML: { name: "Brave", id: "com.brave.Browser" }, BraveBHTML: { name: "Brave Beta", id: "com.brave.Browser.beta" }, BraveSSHTM: { name: "Brave Nightly", id: "com.brave.Browser.nightly" } }, Oe = class extends Error {
  };
});
async function ft() {
  if (ht.platform === "darwin") {
    let r = await at();
    return { name: await lt(r), id: r };
  }
  if (ht.platform === "linux") {
    let { stdout: r } = await wn("xdg-mime", ["query", "default", "x-scheme-handler/http"]), e = r.trim();
    return { name: _n(e.replace(/.desktop$/, "").replace("-", " ")), id: e };
  }
  if (ht.platform === "win32")
    return ct();
  throw new Error("Only macOS, Linux, and Windows are supported");
}
var wn;
var _n;
var hr = P(() => {
  ir();
  lr();
  cr();
  wn = gn(yn), _n = (r) => r.toLowerCase().replaceAll(/(?:^|\s|-)\S/g, (e) => e.toUpperCase());
});
var _r = {};
Gs(_r, { apps: () => D, default: () => Pn, openApp: () => Cn });
async function kn() {
  let r = await ot(), e = String.raw`(Get-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice").ProgId`, t = gr.from(e, "utf16le").toString("base64"), { stdout: s } = await En(r, ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", t], { encoding: "utf8" }), n = s.trim(), i = { ChromeHTML: "com.google.chrome", BraveHTML: "com.brave.Browser", MSEdgeHTM: "com.microsoft.edge", FirefoxURL: "org.mozilla.firefox" };
  return i[n] ? { id: i[n] } : {};
}
function pr(r) {
  if (typeof r == "string" || Array.isArray(r))
    return r;
  let { [ur]: e } = r;
  if (!e)
    throw new Error(`${ur} is not supported`);
  return e;
}
function Ue({ [se]: r }, { wsl: e }) {
  if (e && F)
    return pr(e);
  if (!r)
    throw new Error(`${se} is not supported`);
  return pr(r);
}
var En;
var ut;
var fr;
var se;
var ur;
var dr;
var we;
var Tn;
var Cn;
var D;
var Pn;
var vr = P(() => {
  sr();
  nr();
  hr();
  nt();
  En = Sn(wr.execFile), ut = yr.dirname(vn(import.meta.url)), fr = yr.join(ut, "xdg-open"), { platform: se, arch: ur } = mr;
  dr = async (r, e) => {
    let t;
    for (let s of r)
      try {
        return await e(s);
      } catch (n) {
        t = n;
      }
    throw t;
  }, we = async (r) => {
    if (r = { wait: false, background: false, newInstance: false, allowNonzeroExitCode: false, ...r }, Array.isArray(r.app))
      return dr(r.app, (a) => we({ ...r, app: a }));
    let { name: e, arguments: t = [] } = r.app ?? {};
    if (t = [...t], Array.isArray(e))
      return dr(e, (a) => we({ ...r, app: { name: a, arguments: t } }));
    if (e === "browser" || e === "browserPrivate") {
      let a = { "com.google.chrome": "chrome", "google-chrome.desktop": "chrome", "com.brave.Browser": "brave", "org.mozilla.firefox": "firefox", "firefox.desktop": "firefox", "com.microsoft.msedge": "edge", "com.microsoft.edge": "edge", "com.microsoft.edgemac": "edge", "microsoft-edge.desktop": "edge" }, l = { chrome: "--incognito", brave: "--incognito", firefox: "--private-window", edge: "--inPrivate" }, c = F ? await kn() : await ft();
      if (c.id in a) {
        let h = a[c.id];
        return e === "browserPrivate" && t.push(l[h]), we({ ...r, app: { name: D[h], arguments: t } });
      }
      throw new Error(`${c.name} is not supported as a default browser`);
    }
    let s, n = [], i = {};
    if (se === "darwin")
      s = "open", r.wait && n.push("--wait-apps"), r.background && n.push("--background"), r.newInstance && n.push("--new"), e && n.push("-a", e);
    else if (se === "win32" || F && !re() && !e) {
      s = await ot(), n.push("-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand"), F || (i.windowsVerbatimArguments = true);
      let a = ["Start"];
      r.wait && a.push("-Wait"), e ? (a.push(`"\`"${e}\`""`), r.target && t.push(r.target)) : r.target && a.push(`"${r.target}"`), t.length > 0 && (t = t.map((l) => `"\`"${l}\`""`), a.push("-ArgumentList", t.join(","))), r.target = gr.from(a.join(" "), "utf16le").toString("base64");
    } else {
      if (e)
        s = e;
      else {
        let a = !ut || ut === "/", l = false;
        try {
          await xn.access(fr, bn.X_OK), l = true;
        } catch {}
        s = mr.versions.electron ?? (se === "android" || a || !l) ? "xdg-open" : fr;
      }
      t.length > 0 && n.push(...t), r.wait || (i.stdio = "ignore", i.detached = true);
    }
    se === "darwin" && t.length > 0 && n.push("--args", ...t), r.target && n.push(r.target);
    let o = wr.spawn(s, n, i);
    return r.wait ? new Promise((a, l) => {
      o.once("error", l), o.once("close", (c) => {
        if (!r.allowNonzeroExitCode && c > 0) {
          l(new Error(`Exited with code ${c}`));
          return;
        }
        a(o);
      });
    }) : (o.unref(), o);
  }, Tn = (r, e) => {
    if (typeof r != "string")
      throw new TypeError("Expected a `target`");
    return we({ ...e, target: r });
  }, Cn = (r, e) => {
    if (typeof r != "string" && !Array.isArray(r))
      throw new TypeError("Expected a valid `name`");
    let { arguments: t = [] } = e ?? {};
    if (t != null && !Array.isArray(t))
      throw new TypeError("Expected `appArguments` as Array type");
    return we({ ...e, app: { name: r, arguments: t } });
  };
  D = {};
  M(D, "chrome", () => Ue({ darwin: "google chrome", win32: "chrome", linux: ["google-chrome", "google-chrome-stable", "chromium"] }, { wsl: { ia32: "/mnt/c/Program Files (x86)/Google/Chrome/Application/chrome.exe", x64: ["/mnt/c/Program Files/Google/Chrome/Application/chrome.exe", "/mnt/c/Program Files (x86)/Google/Chrome/Application/chrome.exe"] } }));
  M(D, "brave", () => Ue({ darwin: "brave browser", win32: "brave", linux: ["brave-browser", "brave"] }, { wsl: { ia32: "/mnt/c/Program Files (x86)/BraveSoftware/Brave-Browser/Application/brave.exe", x64: ["/mnt/c/Program Files/BraveSoftware/Brave-Browser/Application/brave.exe", "/mnt/c/Program Files (x86)/BraveSoftware/Brave-Browser/Application/brave.exe"] } }));
  M(D, "firefox", () => Ue({ darwin: "firefox", win32: String.raw`C:\Program Files\Mozilla Firefox\firefox.exe`, linux: "firefox" }, { wsl: "/mnt/c/Program Files/Mozilla Firefox/firefox.exe" }));
  M(D, "edge", () => Ue({ darwin: "microsoft edge", win32: "msedge", linux: ["microsoft-edge", "microsoft-edge-dev"] }, { wsl: "/mnt/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" }));
  M(D, "browser", () => "browser");
  M(D, "browserPrivate", () => "browserPrivate");
  Pn = Tn;
});
var N = b((dl, Rr) => {
  var Or = ["nodebuffer", "arraybuffer", "fragments"], Ur = typeof Blob < "u";
  Ur && Or.push("blob");
  Rr.exports = { BINARY_TYPES: Or, CLOSE_TIMEOUT: 30000, EMPTY_BUFFER: Buffer.alloc(0), GUID: "258EAFA5-E914-47DA-95CA-C5AB0DC85B11", hasBlob: Ur, kForOnEventAttribute: Symbol("kIsForOnEventAttribute"), kListener: Symbol("kListener"), kStatusCode: Symbol("status-code"), kWebSocket: Symbol("websocket"), NOOP: () => {} };
});
var ve = b((pl, We) => {
  var { EMPTY_BUFFER: Kn } = N(), bt = Buffer[Symbol.species];
  function Xn(r, e) {
    if (r.length === 0)
      return Kn;
    if (r.length === 1)
      return r[0];
    let t = Buffer.allocUnsafe(e), s = 0;
    for (let n = 0;n < r.length; n++) {
      let i = r[n];
      t.set(i, s), s += i.length;
    }
    return s < e ? new bt(t.buffer, t.byteOffset, s) : t;
  }
  function Lr(r, e, t, s, n) {
    for (let i = 0;i < n; i++)
      t[s + i] = r[i] ^ e[i & 3];
  }
  function Nr(r, e) {
    for (let t = 0;t < r.length; t++)
      r[t] ^= e[t & 3];
  }
  function Zn(r) {
    return r.length === r.buffer.byteLength ? r.buffer : r.buffer.slice(r.byteOffset, r.byteOffset + r.length);
  }
  function Et(r) {
    if (Et.readOnly = true, Buffer.isBuffer(r))
      return r;
    let e;
    return r instanceof ArrayBuffer ? e = new bt(r) : ArrayBuffer.isView(r) ? e = new bt(r.buffer, r.byteOffset, r.byteLength) : (e = Buffer.from(r), Et.readOnly = false), e;
  }
  We.exports = { concat: Xn, mask: Lr, toArrayBuffer: Zn, toBuffer: Et, unmask: Nr };
  if (!process.env.WS_NO_BUFFER_UTIL)
    try {
      let r = v("bufferutil");
      We.exports.mask = function(e, t, s, n, i) {
        i < 48 ? Lr(e, t, s, n, i) : r.mask(e, t, s, n, i);
      }, We.exports.unmask = function(e, t) {
        e.length < 32 ? Nr(e, t) : r.unmask(e, t);
      };
    } catch {}
});
var Br = b((ml, Wr) => {
  var Ir = Symbol("kDone"), kt = Symbol("kRun"), Tt = class {
    constructor(e) {
      this[Ir] = () => {
        this.pending--, this[kt]();
      }, this.concurrency = e || 1 / 0, this.jobs = [], this.pending = 0;
    }
    add(e) {
      this.jobs.push(e), this[kt]();
    }
    [kt]() {
      if (this.pending !== this.concurrency && this.jobs.length) {
        let e = this.jobs.shift();
        this.pending++, e(this[Ir]);
      }
    }
  };
  Wr.exports = Tt;
});
var xe = b((gl, $r) => {
  var Se = v("zlib"), Fr = ve(), Jn = Br(), { kStatusCode: Mr } = N(), Qn = Buffer[Symbol.species], ei = Buffer.from([0, 0, 255, 255]), Fe = Symbol("permessage-deflate"), I = Symbol("total-length"), ae = Symbol("callback"), j = Symbol("buffers"), le = Symbol("error"), Be, Ct = class {
    constructor(e, t, s) {
      if (this._maxPayload = s | 0, this._options = e || {}, this._threshold = this._options.threshold !== undefined ? this._options.threshold : 1024, this._isServer = !!t, this._deflate = null, this._inflate = null, this.params = null, !Be) {
        let n = this._options.concurrencyLimit !== undefined ? this._options.concurrencyLimit : 10;
        Be = new Jn(n);
      }
    }
    static get extensionName() {
      return "permessage-deflate";
    }
    offer() {
      let e = {};
      return this._options.serverNoContextTakeover && (e.server_no_context_takeover = true), this._options.clientNoContextTakeover && (e.client_no_context_takeover = true), this._options.serverMaxWindowBits && (e.server_max_window_bits = this._options.serverMaxWindowBits), this._options.clientMaxWindowBits ? e.client_max_window_bits = this._options.clientMaxWindowBits : this._options.clientMaxWindowBits == null && (e.client_max_window_bits = true), e;
    }
    accept(e) {
      return e = this.normalizeParams(e), this.params = this._isServer ? this.acceptAsServer(e) : this.acceptAsClient(e), this.params;
    }
    cleanup() {
      if (this._inflate && (this._inflate.close(), this._inflate = null), this._deflate) {
        let e = this._deflate[ae];
        this._deflate.close(), this._deflate = null, e && e(new Error("The deflate stream was closed while data was being processed"));
      }
    }
    acceptAsServer(e) {
      let t = this._options, s = e.find((n) => !(t.serverNoContextTakeover === false && n.server_no_context_takeover || n.server_max_window_bits && (t.serverMaxWindowBits === false || typeof t.serverMaxWindowBits == "number" && t.serverMaxWindowBits > n.server_max_window_bits) || typeof t.clientMaxWindowBits == "number" && !n.client_max_window_bits));
      if (!s)
        throw new Error("None of the extension offers can be accepted");
      return t.serverNoContextTakeover && (s.server_no_context_takeover = true), t.clientNoContextTakeover && (s.client_no_context_takeover = true), typeof t.serverMaxWindowBits == "number" && (s.server_max_window_bits = t.serverMaxWindowBits), typeof t.clientMaxWindowBits == "number" ? s.client_max_window_bits = t.clientMaxWindowBits : (s.client_max_window_bits === true || t.clientMaxWindowBits === false) && delete s.client_max_window_bits, s;
    }
    acceptAsClient(e) {
      let t = e[0];
      if (this._options.clientNoContextTakeover === false && t.client_no_context_takeover)
        throw new Error('Unexpected parameter "client_no_context_takeover"');
      if (!t.client_max_window_bits)
        typeof this._options.clientMaxWindowBits == "number" && (t.client_max_window_bits = this._options.clientMaxWindowBits);
      else if (this._options.clientMaxWindowBits === false || typeof this._options.clientMaxWindowBits == "number" && t.client_max_window_bits > this._options.clientMaxWindowBits)
        throw new Error('Unexpected or invalid parameter "client_max_window_bits"');
      return t;
    }
    normalizeParams(e) {
      return e.forEach((t) => {
        Object.keys(t).forEach((s) => {
          let n = t[s];
          if (n.length > 1)
            throw new Error(`Parameter "${s}" must have only a single value`);
          if (n = n[0], s === "client_max_window_bits") {
            if (n !== true) {
              let i = +n;
              if (!Number.isInteger(i) || i < 8 || i > 15)
                throw new TypeError(`Invalid value for parameter "${s}": ${n}`);
              n = i;
            } else if (!this._isServer)
              throw new TypeError(`Invalid value for parameter "${s}": ${n}`);
          } else if (s === "server_max_window_bits") {
            let i = +n;
            if (!Number.isInteger(i) || i < 8 || i > 15)
              throw new TypeError(`Invalid value for parameter "${s}": ${n}`);
            n = i;
          } else if (s === "client_no_context_takeover" || s === "server_no_context_takeover") {
            if (n !== true)
              throw new TypeError(`Invalid value for parameter "${s}": ${n}`);
          } else
            throw new Error(`Unknown parameter "${s}"`);
          t[s] = n;
        });
      }), e;
    }
    decompress(e, t, s) {
      Be.add((n) => {
        this._decompress(e, t, (i, o) => {
          n(), s(i, o);
        });
      });
    }
    compress(e, t, s) {
      Be.add((n) => {
        this._compress(e, t, (i, o) => {
          n(), s(i, o);
        });
      });
    }
    _decompress(e, t, s) {
      let n = this._isServer ? "client" : "server";
      if (!this._inflate) {
        let i = `${n}_max_window_bits`, o = typeof this.params[i] != "number" ? Se.Z_DEFAULT_WINDOWBITS : this.params[i];
        this._inflate = Se.createInflateRaw({ ...this._options.zlibInflateOptions, windowBits: o }), this._inflate[Fe] = this, this._inflate[I] = 0, this._inflate[j] = [], this._inflate.on("error", ri), this._inflate.on("data", Dr);
      }
      this._inflate[ae] = s, this._inflate.write(e), t && this._inflate.write(ei), this._inflate.flush(() => {
        let i = this._inflate[le];
        if (i) {
          this._inflate.close(), this._inflate = null, s(i);
          return;
        }
        let o = Fr.concat(this._inflate[j], this._inflate[I]);
        this._inflate._readableState.endEmitted ? (this._inflate.close(), this._inflate = null) : (this._inflate[I] = 0, this._inflate[j] = [], t && this.params[`${n}_no_context_takeover`] && this._inflate.reset()), s(null, o);
      });
    }
    _compress(e, t, s) {
      let n = this._isServer ? "server" : "client";
      if (!this._deflate) {
        let i = `${n}_max_window_bits`, o = typeof this.params[i] != "number" ? Se.Z_DEFAULT_WINDOWBITS : this.params[i];
        this._deflate = Se.createDeflateRaw({ ...this._options.zlibDeflateOptions, windowBits: o }), this._deflate[I] = 0, this._deflate[j] = [], this._deflate.on("data", ti);
      }
      this._deflate[ae] = s, this._deflate.write(e), this._deflate.flush(Se.Z_SYNC_FLUSH, () => {
        if (!this._deflate)
          return;
        let i = Fr.concat(this._deflate[j], this._deflate[I]);
        t && (i = new Qn(i.buffer, i.byteOffset, i.length - 4)), this._deflate[ae] = null, this._deflate[I] = 0, this._deflate[j] = [], t && this.params[`${n}_no_context_takeover`] && this._deflate.reset(), s(null, i);
      });
    }
  };
  $r.exports = Ct;
  function ti(r) {
    this[j].push(r), this[I] += r.length;
  }
  function Dr(r) {
    if (this[I] += r.length, this[Fe]._maxPayload < 1 || this[I] <= this[Fe]._maxPayload) {
      this[j].push(r);
      return;
    }
    this[le] = new RangeError("Max payload size exceeded"), this[le].code = "WS_ERR_UNSUPPORTED_MESSAGE_LENGTH", this[le][Mr] = 1009, this.removeListener("data", Dr), this.reset();
  }
  function ri(r) {
    if (this[Fe]._inflate = null, this[le]) {
      this[ae](this[le]);
      return;
    }
    r[Mr] = 1007, this[ae](r);
  }
});
var ce = b((yl, Me) => {
  var { isUtf8: jr } = v("buffer"), { hasBlob: si } = N(), ni = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0];
  function ii(r) {
    return r >= 1000 && r <= 1014 && r !== 1004 && r !== 1005 && r !== 1006 || r >= 3000 && r <= 4999;
  }
  function Pt(r) {
    let e = r.length, t = 0;
    for (;t < e; )
      if ((r[t] & 128) === 0)
        t++;
      else if ((r[t] & 224) === 192) {
        if (t + 1 === e || (r[t + 1] & 192) !== 128 || (r[t] & 254) === 192)
          return false;
        t += 2;
      } else if ((r[t] & 240) === 224) {
        if (t + 2 >= e || (r[t + 1] & 192) !== 128 || (r[t + 2] & 192) !== 128 || r[t] === 224 && (r[t + 1] & 224) === 128 || r[t] === 237 && (r[t + 1] & 224) === 160)
          return false;
        t += 3;
      } else if ((r[t] & 248) === 240) {
        if (t + 3 >= e || (r[t + 1] & 192) !== 128 || (r[t + 2] & 192) !== 128 || (r[t + 3] & 192) !== 128 || r[t] === 240 && (r[t + 1] & 240) === 128 || r[t] === 244 && r[t + 1] > 143 || r[t] > 244)
          return false;
        t += 4;
      } else
        return false;
    return true;
  }
  function oi(r) {
    return si && typeof r == "object" && typeof r.arrayBuffer == "function" && typeof r.type == "string" && typeof r.stream == "function" && (r[Symbol.toStringTag] === "Blob" || r[Symbol.toStringTag] === "File");
  }
  Me.exports = { isBlob: oi, isValidStatusCode: ii, isValidUTF8: Pt, tokenChars: ni };
  if (jr)
    Me.exports.isValidUTF8 = function(r) {
      return r.length < 24 ? Pt(r) : jr(r);
    };
  else if (!process.env.WS_NO_UTF_8_VALIDATE)
    try {
      let r = v("utf-8-validate");
      Me.exports.isValidUTF8 = function(e) {
        return e.length < 32 ? Pt(e) : r(e);
      };
    } catch {}
});
var Lt = b((wl, Kr) => {
  var { Writable: ai } = v("stream"), qr = xe(), { BINARY_TYPES: li, EMPTY_BUFFER: Gr, kStatusCode: ci, kWebSocket: hi } = N(), { concat: At, toArrayBuffer: fi, unmask: ui } = ve(), { isValidStatusCode: di, isValidUTF8: Hr } = ce(), De = Buffer[Symbol.species], T = 0, zr = 1, Vr = 2, Yr = 3, Ot = 4, Ut = 5, $e = 6, Rt = class extends ai {
    constructor(e = {}) {
      super(), this._allowSynchronousEvents = e.allowSynchronousEvents !== undefined ? e.allowSynchronousEvents : true, this._binaryType = e.binaryType || li[0], this._extensions = e.extensions || {}, this._isServer = !!e.isServer, this._maxPayload = e.maxPayload | 0, this._skipUTF8Validation = !!e.skipUTF8Validation, this[hi] = undefined, this._bufferedBytes = 0, this._buffers = [], this._compressed = false, this._payloadLength = 0, this._mask = undefined, this._fragmented = 0, this._masked = false, this._fin = false, this._opcode = 0, this._totalPayloadLength = 0, this._messageLength = 0, this._fragments = [], this._errored = false, this._loop = false, this._state = T;
    }
    _write(e, t, s) {
      if (this._opcode === 8 && this._state == T)
        return s();
      this._bufferedBytes += e.length, this._buffers.push(e), this.startLoop(s);
    }
    consume(e) {
      if (this._bufferedBytes -= e, e === this._buffers[0].length)
        return this._buffers.shift();
      if (e < this._buffers[0].length) {
        let s = this._buffers[0];
        return this._buffers[0] = new De(s.buffer, s.byteOffset + e, s.length - e), new De(s.buffer, s.byteOffset, e);
      }
      let t = Buffer.allocUnsafe(e);
      do {
        let s = this._buffers[0], n = t.length - e;
        e >= s.length ? t.set(this._buffers.shift(), n) : (t.set(new Uint8Array(s.buffer, s.byteOffset, e), n), this._buffers[0] = new De(s.buffer, s.byteOffset + e, s.length - e)), e -= s.length;
      } while (e > 0);
      return t;
    }
    startLoop(e) {
      this._loop = true;
      do
        switch (this._state) {
          case T:
            this.getInfo(e);
            break;
          case zr:
            this.getPayloadLength16(e);
            break;
          case Vr:
            this.getPayloadLength64(e);
            break;
          case Yr:
            this.getMask();
            break;
          case Ot:
            this.getData(e);
            break;
          case Ut:
          case $e:
            this._loop = false;
            return;
        }
      while (this._loop);
      this._errored || e();
    }
    getInfo(e) {
      if (this._bufferedBytes < 2) {
        this._loop = false;
        return;
      }
      let t = this.consume(2);
      if ((t[0] & 48) !== 0) {
        let n = this.createError(RangeError, "RSV2 and RSV3 must be clear", true, 1002, "WS_ERR_UNEXPECTED_RSV_2_3");
        e(n);
        return;
      }
      let s = (t[0] & 64) === 64;
      if (s && !this._extensions[qr.extensionName]) {
        let n = this.createError(RangeError, "RSV1 must be clear", true, 1002, "WS_ERR_UNEXPECTED_RSV_1");
        e(n);
        return;
      }
      if (this._fin = (t[0] & 128) === 128, this._opcode = t[0] & 15, this._payloadLength = t[1] & 127, this._opcode === 0) {
        if (s) {
          let n = this.createError(RangeError, "RSV1 must be clear", true, 1002, "WS_ERR_UNEXPECTED_RSV_1");
          e(n);
          return;
        }
        if (!this._fragmented) {
          let n = this.createError(RangeError, "invalid opcode 0", true, 1002, "WS_ERR_INVALID_OPCODE");
          e(n);
          return;
        }
        this._opcode = this._fragmented;
      } else if (this._opcode === 1 || this._opcode === 2) {
        if (this._fragmented) {
          let n = this.createError(RangeError, `invalid opcode ${this._opcode}`, true, 1002, "WS_ERR_INVALID_OPCODE");
          e(n);
          return;
        }
        this._compressed = s;
      } else if (this._opcode > 7 && this._opcode < 11) {
        if (!this._fin) {
          let n = this.createError(RangeError, "FIN must be set", true, 1002, "WS_ERR_EXPECTED_FIN");
          e(n);
          return;
        }
        if (s) {
          let n = this.createError(RangeError, "RSV1 must be clear", true, 1002, "WS_ERR_UNEXPECTED_RSV_1");
          e(n);
          return;
        }
        if (this._payloadLength > 125 || this._opcode === 8 && this._payloadLength === 1) {
          let n = this.createError(RangeError, `invalid payload length ${this._payloadLength}`, true, 1002, "WS_ERR_INVALID_CONTROL_PAYLOAD_LENGTH");
          e(n);
          return;
        }
      } else {
        let n = this.createError(RangeError, `invalid opcode ${this._opcode}`, true, 1002, "WS_ERR_INVALID_OPCODE");
        e(n);
        return;
      }
      if (!this._fin && !this._fragmented && (this._fragmented = this._opcode), this._masked = (t[1] & 128) === 128, this._isServer) {
        if (!this._masked) {
          let n = this.createError(RangeError, "MASK must be set", true, 1002, "WS_ERR_EXPECTED_MASK");
          e(n);
          return;
        }
      } else if (this._masked) {
        let n = this.createError(RangeError, "MASK must be clear", true, 1002, "WS_ERR_UNEXPECTED_MASK");
        e(n);
        return;
      }
      this._payloadLength === 126 ? this._state = zr : this._payloadLength === 127 ? this._state = Vr : this.haveLength(e);
    }
    getPayloadLength16(e) {
      if (this._bufferedBytes < 2) {
        this._loop = false;
        return;
      }
      this._payloadLength = this.consume(2).readUInt16BE(0), this.haveLength(e);
    }
    getPayloadLength64(e) {
      if (this._bufferedBytes < 8) {
        this._loop = false;
        return;
      }
      let t = this.consume(8), s = t.readUInt32BE(0);
      if (s > Math.pow(2, 21) - 1) {
        let n = this.createError(RangeError, "Unsupported WebSocket frame: payload length > 2^53 - 1", false, 1009, "WS_ERR_UNSUPPORTED_DATA_PAYLOAD_LENGTH");
        e(n);
        return;
      }
      this._payloadLength = s * Math.pow(2, 32) + t.readUInt32BE(4), this.haveLength(e);
    }
    haveLength(e) {
      if (this._payloadLength && this._opcode < 8 && (this._totalPayloadLength += this._payloadLength, this._totalPayloadLength > this._maxPayload && this._maxPayload > 0)) {
        let t = this.createError(RangeError, "Max payload size exceeded", false, 1009, "WS_ERR_UNSUPPORTED_MESSAGE_LENGTH");
        e(t);
        return;
      }
      this._masked ? this._state = Yr : this._state = Ot;
    }
    getMask() {
      if (this._bufferedBytes < 4) {
        this._loop = false;
        return;
      }
      this._mask = this.consume(4), this._state = Ot;
    }
    getData(e) {
      let t = Gr;
      if (this._payloadLength) {
        if (this._bufferedBytes < this._payloadLength) {
          this._loop = false;
          return;
        }
        t = this.consume(this._payloadLength), this._masked && (this._mask[0] | this._mask[1] | this._mask[2] | this._mask[3]) !== 0 && ui(t, this._mask);
      }
      if (this._opcode > 7) {
        this.controlMessage(t, e);
        return;
      }
      if (this._compressed) {
        this._state = Ut, this.decompress(t, e);
        return;
      }
      t.length && (this._messageLength = this._totalPayloadLength, this._fragments.push(t)), this.dataMessage(e);
    }
    decompress(e, t) {
      this._extensions[qr.extensionName].decompress(e, this._fin, (n, i) => {
        if (n)
          return t(n);
        if (i.length) {
          if (this._messageLength += i.length, this._messageLength > this._maxPayload && this._maxPayload > 0) {
            let o = this.createError(RangeError, "Max payload size exceeded", false, 1009, "WS_ERR_UNSUPPORTED_MESSAGE_LENGTH");
            t(o);
            return;
          }
          this._fragments.push(i);
        }
        this.dataMessage(t), this._state === T && this.startLoop(t);
      });
    }
    dataMessage(e) {
      if (!this._fin) {
        this._state = T;
        return;
      }
      let t = this._messageLength, s = this._fragments;
      if (this._totalPayloadLength = 0, this._messageLength = 0, this._fragmented = 0, this._fragments = [], this._opcode === 2) {
        let n;
        this._binaryType === "nodebuffer" ? n = At(s, t) : this._binaryType === "arraybuffer" ? n = fi(At(s, t)) : this._binaryType === "blob" ? n = new Blob(s) : n = s, this._allowSynchronousEvents ? (this.emit("message", n, true), this._state = T) : (this._state = $e, setImmediate(() => {
          this.emit("message", n, true), this._state = T, this.startLoop(e);
        }));
      } else {
        let n = At(s, t);
        if (!this._skipUTF8Validation && !Hr(n)) {
          let i = this.createError(Error, "invalid UTF-8 sequence", true, 1007, "WS_ERR_INVALID_UTF8");
          e(i);
          return;
        }
        this._state === Ut || this._allowSynchronousEvents ? (this.emit("message", n, false), this._state = T) : (this._state = $e, setImmediate(() => {
          this.emit("message", n, false), this._state = T, this.startLoop(e);
        }));
      }
    }
    controlMessage(e, t) {
      if (this._opcode === 8) {
        if (e.length === 0)
          this._loop = false, this.emit("conclude", 1005, Gr), this.end();
        else {
          let s = e.readUInt16BE(0);
          if (!di(s)) {
            let i = this.createError(RangeError, `invalid status code ${s}`, true, 1002, "WS_ERR_INVALID_CLOSE_CODE");
            t(i);
            return;
          }
          let n = new De(e.buffer, e.byteOffset + 2, e.length - 2);
          if (!this._skipUTF8Validation && !Hr(n)) {
            let i = this.createError(Error, "invalid UTF-8 sequence", true, 1007, "WS_ERR_INVALID_UTF8");
            t(i);
            return;
          }
          this._loop = false, this.emit("conclude", s, n), this.end();
        }
        this._state = T;
        return;
      }
      this._allowSynchronousEvents ? (this.emit(this._opcode === 9 ? "ping" : "pong", e), this._state = T) : (this._state = $e, setImmediate(() => {
        this.emit(this._opcode === 9 ? "ping" : "pong", e), this._state = T, this.startLoop(t);
      }));
    }
    createError(e, t, s, n, i) {
      this._loop = false, this._errored = true;
      let o = new e(s ? `Invalid WebSocket frame: ${t}` : t);
      return Error.captureStackTrace(o, this.createError), o.code = i, o[ci] = n, o;
    }
  };
  Kr.exports = Rt;
});
var Wt = b((vl, Jr) => {
  var { Duplex: _l } = v("stream"), { randomFillSync: pi } = v("crypto"), Xr = xe(), { EMPTY_BUFFER: mi, kWebSocket: gi, NOOP: yi } = N(), { isBlob: he, isValidStatusCode: wi } = ce(), { mask: Zr, toBuffer: K } = ve(), C = Symbol("kByteLength"), _i = Buffer.alloc(4), je = 8 * 1024, X, fe = je, O = 0, vi = 1, Si = 2, Nt = class r {
    constructor(e, t, s) {
      this._extensions = t || {}, s && (this._generateMask = s, this._maskBuffer = Buffer.alloc(4)), this._socket = e, this._firstFragment = true, this._compress = false, this._bufferedBytes = 0, this._queue = [], this._state = O, this.onerror = yi, this[gi] = undefined;
    }
    static frame(e, t) {
      let s, n = false, i = 2, o = false;
      t.mask && (s = t.maskBuffer || _i, t.generateMask ? t.generateMask(s) : (fe === je && (X === undefined && (X = Buffer.alloc(je)), pi(X, 0, je), fe = 0), s[0] = X[fe++], s[1] = X[fe++], s[2] = X[fe++], s[3] = X[fe++]), o = (s[0] | s[1] | s[2] | s[3]) === 0, i = 6);
      let a;
      typeof e == "string" ? (!t.mask || o) && t[C] !== undefined ? a = t[C] : (e = Buffer.from(e), a = e.length) : (a = e.length, n = t.mask && t.readOnly && !o);
      let l = a;
      a >= 65536 ? (i += 8, l = 127) : a > 125 && (i += 2, l = 126);
      let c = Buffer.allocUnsafe(n ? a + i : i);
      return c[0] = t.fin ? t.opcode | 128 : t.opcode, t.rsv1 && (c[0] |= 64), c[1] = l, l === 126 ? c.writeUInt16BE(a, 2) : l === 127 && (c[2] = c[3] = 0, c.writeUIntBE(a, 4, 6)), t.mask ? (c[1] |= 128, c[i - 4] = s[0], c[i - 3] = s[1], c[i - 2] = s[2], c[i - 1] = s[3], o ? [c, e] : n ? (Zr(e, s, c, i, a), [c]) : (Zr(e, s, e, 0, a), [c, e])) : [c, e];
    }
    close(e, t, s, n) {
      let i;
      if (e === undefined)
        i = mi;
      else {
        if (typeof e != "number" || !wi(e))
          throw new TypeError("First argument must be a valid error code number");
        if (t === undefined || !t.length)
          i = Buffer.allocUnsafe(2), i.writeUInt16BE(e, 0);
        else {
          let a = Buffer.byteLength(t);
          if (a > 123)
            throw new RangeError("The message must not be greater than 123 bytes");
          i = Buffer.allocUnsafe(2 + a), i.writeUInt16BE(e, 0), typeof t == "string" ? i.write(t, 2) : i.set(t, 2);
        }
      }
      let o = { [C]: i.length, fin: true, generateMask: this._generateMask, mask: s, maskBuffer: this._maskBuffer, opcode: 8, readOnly: false, rsv1: false };
      this._state !== O ? this.enqueue([this.dispatch, i, false, o, n]) : this.sendFrame(r.frame(i, o), n);
    }
    ping(e, t, s) {
      let n, i;
      if (typeof e == "string" ? (n = Buffer.byteLength(e), i = false) : he(e) ? (n = e.size, i = false) : (e = K(e), n = e.length, i = K.readOnly), n > 125)
        throw new RangeError("The data size must not be greater than 125 bytes");
      let o = { [C]: n, fin: true, generateMask: this._generateMask, mask: t, maskBuffer: this._maskBuffer, opcode: 9, readOnly: i, rsv1: false };
      he(e) ? this._state !== O ? this.enqueue([this.getBlobData, e, false, o, s]) : this.getBlobData(e, false, o, s) : this._state !== O ? this.enqueue([this.dispatch, e, false, o, s]) : this.sendFrame(r.frame(e, o), s);
    }
    pong(e, t, s) {
      let n, i;
      if (typeof e == "string" ? (n = Buffer.byteLength(e), i = false) : he(e) ? (n = e.size, i = false) : (e = K(e), n = e.length, i = K.readOnly), n > 125)
        throw new RangeError("The data size must not be greater than 125 bytes");
      let o = { [C]: n, fin: true, generateMask: this._generateMask, mask: t, maskBuffer: this._maskBuffer, opcode: 10, readOnly: i, rsv1: false };
      he(e) ? this._state !== O ? this.enqueue([this.getBlobData, e, false, o, s]) : this.getBlobData(e, false, o, s) : this._state !== O ? this.enqueue([this.dispatch, e, false, o, s]) : this.sendFrame(r.frame(e, o), s);
    }
    send(e, t, s) {
      let n = this._extensions[Xr.extensionName], i = t.binary ? 2 : 1, o = t.compress, a, l;
      typeof e == "string" ? (a = Buffer.byteLength(e), l = false) : he(e) ? (a = e.size, l = false) : (e = K(e), a = e.length, l = K.readOnly), this._firstFragment ? (this._firstFragment = false, o && n && n.params[n._isServer ? "server_no_context_takeover" : "client_no_context_takeover"] && (o = a >= n._threshold), this._compress = o) : (o = false, i = 0), t.fin && (this._firstFragment = true);
      let c = { [C]: a, fin: t.fin, generateMask: this._generateMask, mask: t.mask, maskBuffer: this._maskBuffer, opcode: i, readOnly: l, rsv1: o };
      he(e) ? this._state !== O ? this.enqueue([this.getBlobData, e, this._compress, c, s]) : this.getBlobData(e, this._compress, c, s) : this._state !== O ? this.enqueue([this.dispatch, e, this._compress, c, s]) : this.dispatch(e, this._compress, c, s);
    }
    getBlobData(e, t, s, n) {
      this._bufferedBytes += s[C], this._state = Si, e.arrayBuffer().then((i) => {
        if (this._socket.destroyed) {
          let a = new Error("The socket was closed while the blob was being read");
          process.nextTick(It, this, a, n);
          return;
        }
        this._bufferedBytes -= s[C];
        let o = K(i);
        t ? this.dispatch(o, t, s, n) : (this._state = O, this.sendFrame(r.frame(o, s), n), this.dequeue());
      }).catch((i) => {
        process.nextTick(xi, this, i, n);
      });
    }
    dispatch(e, t, s, n) {
      if (!t) {
        this.sendFrame(r.frame(e, s), n);
        return;
      }
      let i = this._extensions[Xr.extensionName];
      this._bufferedBytes += s[C], this._state = vi, i.compress(e, s.fin, (o, a) => {
        if (this._socket.destroyed) {
          let l = new Error("The socket was closed while data was being compressed");
          It(this, l, n);
          return;
        }
        this._bufferedBytes -= s[C], this._state = O, s.readOnly = false, this.sendFrame(r.frame(a, s), n), this.dequeue();
      });
    }
    dequeue() {
      for (;this._state === O && this._queue.length; ) {
        let e = this._queue.shift();
        this._bufferedBytes -= e[3][C], Reflect.apply(e[0], this, e.slice(1));
      }
    }
    enqueue(e) {
      this._bufferedBytes += e[3][C], this._queue.push(e);
    }
    sendFrame(e, t) {
      e.length === 2 ? (this._socket.cork(), this._socket.write(e[0]), this._socket.write(e[1], t), this._socket.uncork()) : this._socket.write(e[0], t);
    }
  };
  Jr.exports = Nt;
  function It(r, e, t) {
    typeof t == "function" && t(e);
    for (let s = 0;s < r._queue.length; s++) {
      let n = r._queue[s], i = n[n.length - 1];
      typeof i == "function" && i(e);
    }
  }
  function xi(r, e, t) {
    It(r, e, t), r.onerror(e);
  }
});
var as = b((Sl, os) => {
  var { kForOnEventAttribute: be, kListener: Bt } = N(), Qr = Symbol("kCode"), es = Symbol("kData"), ts = Symbol("kError"), rs = Symbol("kMessage"), ss = Symbol("kReason"), ue = Symbol("kTarget"), ns = Symbol("kType"), is = Symbol("kWasClean"), W = class {
    constructor(e) {
      this[ue] = null, this[ns] = e;
    }
    get target() {
      return this[ue];
    }
    get type() {
      return this[ns];
    }
  };
  Object.defineProperty(W.prototype, "target", { enumerable: true });
  Object.defineProperty(W.prototype, "type", { enumerable: true });
  var Z = class extends W {
    constructor(e, t = {}) {
      super(e), this[Qr] = t.code === undefined ? 0 : t.code, this[ss] = t.reason === undefined ? "" : t.reason, this[is] = t.wasClean === undefined ? false : t.wasClean;
    }
    get code() {
      return this[Qr];
    }
    get reason() {
      return this[ss];
    }
    get wasClean() {
      return this[is];
    }
  };
  Object.defineProperty(Z.prototype, "code", { enumerable: true });
  Object.defineProperty(Z.prototype, "reason", { enumerable: true });
  Object.defineProperty(Z.prototype, "wasClean", { enumerable: true });
  var de = class extends W {
    constructor(e, t = {}) {
      super(e), this[ts] = t.error === undefined ? null : t.error, this[rs] = t.message === undefined ? "" : t.message;
    }
    get error() {
      return this[ts];
    }
    get message() {
      return this[rs];
    }
  };
  Object.defineProperty(de.prototype, "error", { enumerable: true });
  Object.defineProperty(de.prototype, "message", { enumerable: true });
  var Ee = class extends W {
    constructor(e, t = {}) {
      super(e), this[es] = t.data === undefined ? null : t.data;
    }
    get data() {
      return this[es];
    }
  };
  Object.defineProperty(Ee.prototype, "data", { enumerable: true });
  var bi = { addEventListener(r, e, t = {}) {
    for (let n of this.listeners(r))
      if (!t[be] && n[Bt] === e && !n[be])
        return;
    let s;
    if (r === "message")
      s = function(i, o) {
        let a = new Ee("message", { data: o ? i : i.toString() });
        a[ue] = this, qe(e, this, a);
      };
    else if (r === "close")
      s = function(i, o) {
        let a = new Z("close", { code: i, reason: o.toString(), wasClean: this._closeFrameReceived && this._closeFrameSent });
        a[ue] = this, qe(e, this, a);
      };
    else if (r === "error")
      s = function(i) {
        let o = new de("error", { error: i, message: i.message });
        o[ue] = this, qe(e, this, o);
      };
    else if (r === "open")
      s = function() {
        let i = new W("open");
        i[ue] = this, qe(e, this, i);
      };
    else
      return;
    s[be] = !!t[be], s[Bt] = e, t.once ? this.once(r, s) : this.on(r, s);
  }, removeEventListener(r, e) {
    for (let t of this.listeners(r))
      if (t[Bt] === e && !t[be]) {
        this.removeListener(r, t);
        break;
      }
  } };
  os.exports = { CloseEvent: Z, ErrorEvent: de, Event: W, EventTarget: bi, MessageEvent: Ee };
  function qe(r, e, t) {
    typeof r == "object" && r.handleEvent ? r.handleEvent.call(r, t) : r.call(e, t);
  }
});
var Ft = b((xl, ls) => {
  var { tokenChars: ke } = ce();
  function R(r, e, t) {
    r[e] === undefined ? r[e] = [t] : r[e].push(t);
  }
  function Ei(r) {
    let e = Object.create(null), t = Object.create(null), s = false, n = false, i = false, o, a, l = -1, c = -1, h = -1, f = 0;
    for (;f < r.length; f++)
      if (c = r.charCodeAt(f), o === undefined)
        if (h === -1 && ke[c] === 1)
          l === -1 && (l = f);
        else if (f !== 0 && (c === 32 || c === 9))
          h === -1 && l !== -1 && (h = f);
        else if (c === 59 || c === 44) {
          if (l === -1)
            throw new SyntaxError(`Unexpected character at index ${f}`);
          h === -1 && (h = f);
          let _ = r.slice(l, h);
          c === 44 ? (R(e, _, t), t = Object.create(null)) : o = _, l = h = -1;
        } else
          throw new SyntaxError(`Unexpected character at index ${f}`);
      else if (a === undefined)
        if (h === -1 && ke[c] === 1)
          l === -1 && (l = f);
        else if (c === 32 || c === 9)
          h === -1 && l !== -1 && (h = f);
        else if (c === 59 || c === 44) {
          if (l === -1)
            throw new SyntaxError(`Unexpected character at index ${f}`);
          h === -1 && (h = f), R(t, r.slice(l, h), true), c === 44 && (R(e, o, t), t = Object.create(null), o = undefined), l = h = -1;
        } else if (c === 61 && l !== -1 && h === -1)
          a = r.slice(l, f), l = h = -1;
        else
          throw new SyntaxError(`Unexpected character at index ${f}`);
      else if (n) {
        if (ke[c] !== 1)
          throw new SyntaxError(`Unexpected character at index ${f}`);
        l === -1 ? l = f : s || (s = true), n = false;
      } else if (i)
        if (ke[c] === 1)
          l === -1 && (l = f);
        else if (c === 34 && l !== -1)
          i = false, h = f;
        else if (c === 92)
          n = true;
        else
          throw new SyntaxError(`Unexpected character at index ${f}`);
      else if (c === 34 && r.charCodeAt(f - 1) === 61)
        i = true;
      else if (h === -1 && ke[c] === 1)
        l === -1 && (l = f);
      else if (l !== -1 && (c === 32 || c === 9))
        h === -1 && (h = f);
      else if (c === 59 || c === 44) {
        if (l === -1)
          throw new SyntaxError(`Unexpected character at index ${f}`);
        h === -1 && (h = f);
        let _ = r.slice(l, h);
        s && (_ = _.replace(/\\/g, ""), s = false), R(t, a, _), c === 44 && (R(e, o, t), t = Object.create(null), o = undefined), a = undefined, l = h = -1;
      } else
        throw new SyntaxError(`Unexpected character at index ${f}`);
    if (l === -1 || i || c === 32 || c === 9)
      throw new SyntaxError("Unexpected end of input");
    h === -1 && (h = f);
    let d = r.slice(l, h);
    return o === undefined ? R(e, d, t) : (a === undefined ? R(t, d, true) : s ? R(t, a, d.replace(/\\/g, "")) : R(t, a, d), R(e, o, t)), e;
  }
  function ki(r) {
    return Object.keys(r).map((e) => {
      let t = r[e];
      return Array.isArray(t) || (t = [t]), t.map((s) => [e].concat(Object.keys(s).map((n) => {
        let i = s[n];
        return Array.isArray(i) || (i = [i]), i.map((o) => o === true ? n : `${n}=${o}`).join("; ");
      })).join("; ")).join(", ");
    }).join(", ");
  }
  ls.exports = { format: ki, parse: Ei };
});
var Ve = b((kl, vs) => {
  var Ti = v("events"), Ci = v("https"), Pi = v("http"), fs = v("net"), Ai = v("tls"), { randomBytes: Oi, createHash: Ui } = v("crypto"), { Duplex: bl, Readable: El } = v("stream"), { URL: Mt } = v("url"), q = xe(), Ri = Lt(), Li = Wt(), { isBlob: Ni } = ce(), { BINARY_TYPES: cs, CLOSE_TIMEOUT: Ii, EMPTY_BUFFER: Ge, GUID: Wi, kForOnEventAttribute: Dt, kListener: Bi, kStatusCode: Fi, kWebSocket: x, NOOP: us } = N(), { EventTarget: { addEventListener: Mi, removeEventListener: Di } } = as(), { format: $i, parse: ji } = Ft(), { toBuffer: qi } = ve(), ds = Symbol("kAborted"), $t = [8, 13], B = ["CONNECTING", "OPEN", "CLOSING", "CLOSED"], Gi = /^[!#$%&'*+\-.0-9A-Z^_`|a-z~]+$/, g = class r extends Ti {
    constructor(e, t, s) {
      super(), this._binaryType = cs[0], this._closeCode = 1006, this._closeFrameReceived = false, this._closeFrameSent = false, this._closeMessage = Ge, this._closeTimer = null, this._errorEmitted = false, this._extensions = {}, this._paused = false, this._protocol = "", this._readyState = r.CONNECTING, this._receiver = null, this._sender = null, this._socket = null, e !== null ? (this._bufferedAmount = 0, this._isServer = false, this._redirects = 0, t === undefined ? t = [] : Array.isArray(t) || (typeof t == "object" && t !== null ? (s = t, t = []) : t = [t]), ps(this, e, t, s)) : (this._autoPong = s.autoPong, this._closeTimeout = s.closeTimeout, this._isServer = true);
    }
    get binaryType() {
      return this._binaryType;
    }
    set binaryType(e) {
      cs.includes(e) && (this._binaryType = e, this._receiver && (this._receiver._binaryType = e));
    }
    get bufferedAmount() {
      return this._socket ? this._socket._writableState.length + this._sender._bufferedBytes : this._bufferedAmount;
    }
    get extensions() {
      return Object.keys(this._extensions).join();
    }
    get isPaused() {
      return this._paused;
    }
    get onclose() {
      return null;
    }
    get onerror() {
      return null;
    }
    get onopen() {
      return null;
    }
    get onmessage() {
      return null;
    }
    get protocol() {
      return this._protocol;
    }
    get readyState() {
      return this._readyState;
    }
    get url() {
      return this._url;
    }
    setSocket(e, t, s) {
      let n = new Ri({ allowSynchronousEvents: s.allowSynchronousEvents, binaryType: this.binaryType, extensions: this._extensions, isServer: this._isServer, maxPayload: s.maxPayload, skipUTF8Validation: s.skipUTF8Validation }), i = new Li(e, this._extensions, s.generateMask);
      this._receiver = n, this._sender = i, this._socket = e, n[x] = this, i[x] = this, e[x] = this, n.on("conclude", Vi), n.on("drain", Yi), n.on("error", Ki), n.on("message", Xi), n.on("ping", Zi), n.on("pong", Ji), i.onerror = Qi, e.setTimeout && e.setTimeout(0), e.setNoDelay && e.setNoDelay(), t.length > 0 && e.unshift(t), e.on("close", ys), e.on("data", ze), e.on("end", ws), e.on("error", _s), this._readyState = r.OPEN, this.emit("open");
    }
    emitClose() {
      if (!this._socket) {
        this._readyState = r.CLOSED, this.emit("close", this._closeCode, this._closeMessage);
        return;
      }
      this._extensions[q.extensionName] && this._extensions[q.extensionName].cleanup(), this._receiver.removeAllListeners(), this._readyState = r.CLOSED, this.emit("close", this._closeCode, this._closeMessage);
    }
    close(e, t) {
      if (this.readyState !== r.CLOSED) {
        if (this.readyState === r.CONNECTING) {
          E(this, this._req, "WebSocket was closed before the connection was established");
          return;
        }
        if (this.readyState === r.CLOSING) {
          this._closeFrameSent && (this._closeFrameReceived || this._receiver._writableState.errorEmitted) && this._socket.end();
          return;
        }
        this._readyState = r.CLOSING, this._sender.close(e, t, !this._isServer, (s) => {
          s || (this._closeFrameSent = true, (this._closeFrameReceived || this._receiver._writableState.errorEmitted) && this._socket.end());
        }), gs(this);
      }
    }
    pause() {
      this.readyState === r.CONNECTING || this.readyState === r.CLOSED || (this._paused = true, this._socket.pause());
    }
    ping(e, t, s) {
      if (this.readyState === r.CONNECTING)
        throw new Error("WebSocket is not open: readyState 0 (CONNECTING)");
      if (typeof e == "function" ? (s = e, e = t = undefined) : typeof t == "function" && (s = t, t = undefined), typeof e == "number" && (e = e.toString()), this.readyState !== r.OPEN) {
        jt(this, e, s);
        return;
      }
      t === undefined && (t = !this._isServer), this._sender.ping(e || Ge, t, s);
    }
    pong(e, t, s) {
      if (this.readyState === r.CONNECTING)
        throw new Error("WebSocket is not open: readyState 0 (CONNECTING)");
      if (typeof e == "function" ? (s = e, e = t = undefined) : typeof t == "function" && (s = t, t = undefined), typeof e == "number" && (e = e.toString()), this.readyState !== r.OPEN) {
        jt(this, e, s);
        return;
      }
      t === undefined && (t = !this._isServer), this._sender.pong(e || Ge, t, s);
    }
    resume() {
      this.readyState === r.CONNECTING || this.readyState === r.CLOSED || (this._paused = false, this._receiver._writableState.needDrain || this._socket.resume());
    }
    send(e, t, s) {
      if (this.readyState === r.CONNECTING)
        throw new Error("WebSocket is not open: readyState 0 (CONNECTING)");
      if (typeof t == "function" && (s = t, t = {}), typeof e == "number" && (e = e.toString()), this.readyState !== r.OPEN) {
        jt(this, e, s);
        return;
      }
      let n = { binary: typeof e != "string", mask: !this._isServer, compress: true, fin: true, ...t };
      this._extensions[q.extensionName] || (n.compress = false), this._sender.send(e || Ge, n, s);
    }
    terminate() {
      if (this.readyState !== r.CLOSED) {
        if (this.readyState === r.CONNECTING) {
          E(this, this._req, "WebSocket was closed before the connection was established");
          return;
        }
        this._socket && (this._readyState = r.CLOSING, this._socket.destroy());
      }
    }
  };
  Object.defineProperty(g, "CONNECTING", { enumerable: true, value: B.indexOf("CONNECTING") });
  Object.defineProperty(g.prototype, "CONNECTING", { enumerable: true, value: B.indexOf("CONNECTING") });
  Object.defineProperty(g, "OPEN", { enumerable: true, value: B.indexOf("OPEN") });
  Object.defineProperty(g.prototype, "OPEN", { enumerable: true, value: B.indexOf("OPEN") });
  Object.defineProperty(g, "CLOSING", { enumerable: true, value: B.indexOf("CLOSING") });
  Object.defineProperty(g.prototype, "CLOSING", { enumerable: true, value: B.indexOf("CLOSING") });
  Object.defineProperty(g, "CLOSED", { enumerable: true, value: B.indexOf("CLOSED") });
  Object.defineProperty(g.prototype, "CLOSED", { enumerable: true, value: B.indexOf("CLOSED") });
  ["binaryType", "bufferedAmount", "extensions", "isPaused", "protocol", "readyState", "url"].forEach((r) => {
    Object.defineProperty(g.prototype, r, { enumerable: true });
  });
  ["open", "error", "close", "message"].forEach((r) => {
    Object.defineProperty(g.prototype, `on${r}`, { enumerable: true, get() {
      for (let e of this.listeners(r))
        if (e[Dt])
          return e[Bi];
      return null;
    }, set(e) {
      for (let t of this.listeners(r))
        if (t[Dt]) {
          this.removeListener(r, t);
          break;
        }
      typeof e == "function" && this.addEventListener(r, e, { [Dt]: true });
    } });
  });
  g.prototype.addEventListener = Mi;
  g.prototype.removeEventListener = Di;
  vs.exports = g;
  function ps(r, e, t, s) {
    let n = { allowSynchronousEvents: true, autoPong: true, closeTimeout: Ii, protocolVersion: $t[1], maxPayload: 104857600, skipUTF8Validation: false, perMessageDeflate: true, followRedirects: false, maxRedirects: 10, ...s, socketPath: undefined, hostname: undefined, protocol: undefined, timeout: undefined, method: "GET", host: undefined, path: undefined, port: undefined };
    if (r._autoPong = n.autoPong, r._closeTimeout = n.closeTimeout, !$t.includes(n.protocolVersion))
      throw new RangeError(`Unsupported protocol version: ${n.protocolVersion} (supported versions: ${$t.join(", ")})`);
    let i;
    if (e instanceof Mt)
      i = e;
    else
      try {
        i = new Mt(e);
      } catch {
        throw new SyntaxError(`Invalid URL: ${e}`);
      }
    i.protocol === "http:" ? i.protocol = "ws:" : i.protocol === "https:" && (i.protocol = "wss:"), r._url = i.href;
    let o = i.protocol === "wss:", a = i.protocol === "ws+unix:", l;
    if (i.protocol !== "ws:" && !o && !a ? l = `The URL's protocol must be one of "ws:", "wss:", "http:", "https:", or "ws+unix:"` : a && !i.pathname ? l = "The URL's pathname is empty" : i.hash && (l = "The URL contains a fragment identifier"), l) {
      let p = new SyntaxError(l);
      if (r._redirects === 0)
        throw p;
      He(r, p);
      return;
    }
    let c = o ? 443 : 80, h = Oi(16).toString("base64"), f = o ? Ci.request : Pi.request, d = new Set, _;
    if (n.createConnection = n.createConnection || (o ? zi : Hi), n.defaultPort = n.defaultPort || c, n.port = i.port || c, n.host = i.hostname.startsWith("[") ? i.hostname.slice(1, -1) : i.hostname, n.headers = { ...n.headers, "Sec-WebSocket-Version": n.protocolVersion, "Sec-WebSocket-Key": h, Connection: "Upgrade", Upgrade: "websocket" }, n.path = i.pathname + i.search, n.timeout = n.handshakeTimeout, n.perMessageDeflate && (_ = new q(n.perMessageDeflate !== true ? n.perMessageDeflate : {}, false, n.maxPayload), n.headers["Sec-WebSocket-Extensions"] = $i({ [q.extensionName]: _.offer() })), t.length) {
      for (let p of t) {
        if (typeof p != "string" || !Gi.test(p) || d.has(p))
          throw new SyntaxError("An invalid or duplicated subprotocol was specified");
        d.add(p);
      }
      n.headers["Sec-WebSocket-Protocol"] = t.join(",");
    }
    if (n.origin && (n.protocolVersion < 13 ? n.headers["Sec-WebSocket-Origin"] = n.origin : n.headers.Origin = n.origin), (i.username || i.password) && (n.auth = `${i.username}:${i.password}`), a) {
      let p = n.path.split(":");
      n.socketPath = p[0], n.path = p[1];
    }
    let y;
    if (n.followRedirects) {
      if (r._redirects === 0) {
        r._originalIpc = a, r._originalSecure = o, r._originalHostOrSocketPath = a ? n.socketPath : i.host;
        let p = s && s.headers;
        if (s = { ...s, headers: {} }, p)
          for (let [S, ee] of Object.entries(p))
            s.headers[S.toLowerCase()] = ee;
      } else if (r.listenerCount("redirect") === 0) {
        let p = a ? r._originalIpc ? n.socketPath === r._originalHostOrSocketPath : false : r._originalIpc ? false : i.host === r._originalHostOrSocketPath;
        (!p || r._originalSecure && !o) && (delete n.headers.authorization, delete n.headers.cookie, p || delete n.headers.host, n.auth = undefined);
      }
      n.auth && !s.headers.authorization && (s.headers.authorization = "Basic " + Buffer.from(n.auth).toString("base64")), y = r._req = f(n), r._redirects && r.emit("redirect", r.url, y);
    } else
      y = r._req = f(n);
    n.timeout && y.on("timeout", () => {
      E(r, y, "Opening handshake has timed out");
    }), y.on("error", (p) => {
      y === null || y[ds] || (y = r._req = null, He(r, p));
    }), y.on("response", (p) => {
      let S = p.headers.location, ee = p.statusCode;
      if (S && n.followRedirects && ee >= 300 && ee < 400) {
        if (++r._redirects > n.maxRedirects) {
          E(r, y, "Maximum redirects exceeded");
          return;
        }
        y.abort();
        let pe;
        try {
          pe = new Mt(S, e);
        } catch {
          let te = new SyntaxError(`Invalid URL: ${S}`);
          He(r, te);
          return;
        }
        ps(r, pe, t, s);
      } else
        r.emit("unexpected-response", y, p) || E(r, y, `Unexpected server response: ${p.statusCode}`);
    }), y.on("upgrade", (p, S, ee) => {
      if (r.emit("upgrade", p), r.readyState !== g.CONNECTING)
        return;
      y = r._req = null;
      let pe = p.headers.upgrade;
      if (pe === undefined || pe.toLowerCase() !== "websocket") {
        E(r, S, "Invalid Upgrade header");
        return;
      }
      let Ht = Ui("sha1").update(h + Wi).digest("base64");
      if (p.headers["sec-websocket-accept"] !== Ht) {
        E(r, S, "Invalid Sec-WebSocket-Accept header");
        return;
      }
      let te = p.headers["sec-websocket-protocol"], me;
      if (te !== undefined ? d.size ? d.has(te) || (me = "Server sent an invalid subprotocol") : me = "Server sent a subprotocol but none was requested" : d.size && (me = "Server sent no subprotocol"), me) {
        E(r, S, me);
        return;
      }
      te && (r._protocol = te);
      let zt = p.headers["sec-websocket-extensions"];
      if (zt !== undefined) {
        if (!_) {
          E(r, S, "Server sent a Sec-WebSocket-Extensions header but no extension was requested");
          return;
        }
        let Ke;
        try {
          Ke = ji(zt);
        } catch {
          E(r, S, "Invalid Sec-WebSocket-Extensions header");
          return;
        }
        let Vt = Object.keys(Ke);
        if (Vt.length !== 1 || Vt[0] !== q.extensionName) {
          E(r, S, "Server indicated an extension that was not requested");
          return;
        }
        try {
          _.accept(Ke[q.extensionName]);
        } catch {
          E(r, S, "Invalid Sec-WebSocket-Extensions header");
          return;
        }
        r._extensions[q.extensionName] = _;
      }
      r.setSocket(S, ee, { allowSynchronousEvents: n.allowSynchronousEvents, generateMask: n.generateMask, maxPayload: n.maxPayload, skipUTF8Validation: n.skipUTF8Validation });
    }), n.finishRequest ? n.finishRequest(y, r) : y.end();
  }
  function He(r, e) {
    r._readyState = g.CLOSING, r._errorEmitted = true, r.emit("error", e), r.emitClose();
  }
  function Hi(r) {
    return r.path = r.socketPath, fs.connect(r);
  }
  function zi(r) {
    return r.path = undefined, !r.servername && r.servername !== "" && (r.servername = fs.isIP(r.host) ? "" : r.host), Ai.connect(r);
  }
  function E(r, e, t) {
    r._readyState = g.CLOSING;
    let s = new Error(t);
    Error.captureStackTrace(s, E), e.setHeader ? (e[ds] = true, e.abort(), e.socket && !e.socket.destroyed && e.socket.destroy(), process.nextTick(He, r, s)) : (e.destroy(s), e.once("error", r.emit.bind(r, "error")), e.once("close", r.emitClose.bind(r)));
  }
  function jt(r, e, t) {
    if (e) {
      let s = Ni(e) ? e.size : qi(e).length;
      r._socket ? r._sender._bufferedBytes += s : r._bufferedAmount += s;
    }
    if (t) {
      let s = new Error(`WebSocket is not open: readyState ${r.readyState} (${B[r.readyState]})`);
      process.nextTick(t, s);
    }
  }
  function Vi(r, e) {
    let t = this[x];
    t._closeFrameReceived = true, t._closeMessage = e, t._closeCode = r, t._socket[x] !== undefined && (t._socket.removeListener("data", ze), process.nextTick(ms, t._socket), r === 1005 ? t.close() : t.close(r, e));
  }
  function Yi() {
    let r = this[x];
    r.isPaused || r._socket.resume();
  }
  function Ki(r) {
    let e = this[x];
    e._socket[x] !== undefined && (e._socket.removeListener("data", ze), process.nextTick(ms, e._socket), e.close(r[Fi])), e._errorEmitted || (e._errorEmitted = true, e.emit("error", r));
  }
  function hs() {
    this[x].emitClose();
  }
  function Xi(r, e) {
    this[x].emit("message", r, e);
  }
  function Zi(r) {
    let e = this[x];
    e._autoPong && e.pong(r, !this._isServer, us), e.emit("ping", r);
  }
  function Ji(r) {
    this[x].emit("pong", r);
  }
  function ms(r) {
    r.resume();
  }
  function Qi(r) {
    let e = this[x];
    e.readyState !== g.CLOSED && (e.readyState === g.OPEN && (e._readyState = g.CLOSING, gs(e)), this._socket.end(), e._errorEmitted || (e._errorEmitted = true, e.emit("error", r)));
  }
  function gs(r) {
    r._closeTimer = setTimeout(r._socket.destroy.bind(r._socket), r._closeTimeout);
  }
  function ys() {
    let r = this[x];
    if (this.removeListener("close", ys), this.removeListener("data", ze), this.removeListener("end", ws), r._readyState = g.CLOSING, !this._readableState.endEmitted && !r._closeFrameReceived && !r._receiver._writableState.errorEmitted && this._readableState.length !== 0) {
      let e = this.read(this._readableState.length);
      r._receiver.write(e);
    }
    r._receiver.end(), this[x] = undefined, clearTimeout(r._closeTimer), r._receiver._writableState.finished || r._receiver._writableState.errorEmitted ? r.emitClose() : (r._receiver.on("error", hs), r._receiver.on("finish", hs));
  }
  function ze(r) {
    this[x]._receiver.write(r) || this.pause();
  }
  function ws() {
    let r = this[x];
    r._readyState = g.CLOSING, r._receiver.end(), this.end();
  }
  function _s() {
    let r = this[x];
    this.removeListener("error", _s), this.on("error", us), r && (r._readyState = g.CLOSING, this.destroy());
  }
});
var Es = b((Cl, bs) => {
  var Tl = Ve(), { Duplex: eo } = v("stream");
  function Ss(r) {
    r.emit("close");
  }
  function to() {
    !this.destroyed && this._writableState.finished && this.destroy();
  }
  function xs(r) {
    this.removeListener("error", xs), this.destroy(), this.listenerCount("error") === 0 && this.emit("error", r);
  }
  function ro(r, e) {
    let t = true, s = new eo({ ...e, autoDestroy: false, emitClose: false, objectMode: false, writableObjectMode: false });
    return r.on("message", function(i, o) {
      let a = !o && s._readableState.objectMode ? i.toString() : i;
      s.push(a) || r.pause();
    }), r.once("error", function(i) {
      s.destroyed || (t = false, s.destroy(i));
    }), r.once("close", function() {
      s.destroyed || s.push(null);
    }), s._destroy = function(n, i) {
      if (r.readyState === r.CLOSED) {
        i(n), process.nextTick(Ss, s);
        return;
      }
      let o = false;
      r.once("error", function(l) {
        o = true, i(l);
      }), r.once("close", function() {
        o || i(n), process.nextTick(Ss, s);
      }), t && r.terminate();
    }, s._final = function(n) {
      if (r.readyState === r.CONNECTING) {
        r.once("open", function() {
          s._final(n);
        });
        return;
      }
      r._socket !== null && (r._socket._writableState.finished ? (n(), s._readableState.endEmitted && s.destroy()) : (r._socket.once("finish", function() {
        n();
      }), r.close()));
    }, s._read = function() {
      r.isPaused && r.resume();
    }, s._write = function(n, i, o) {
      if (r.readyState === r.CONNECTING) {
        r.once("open", function() {
          s._write(n, i, o);
        });
        return;
      }
      r.send(n, o);
    }, s.on("end", to), s.on("error", xs), s;
  }
  bs.exports = ro;
});
var Ts = b((Pl, ks) => {
  var { tokenChars: so } = ce();
  function no(r) {
    let e = new Set, t = -1, s = -1, n = 0;
    for (n;n < r.length; n++) {
      let o = r.charCodeAt(n);
      if (s === -1 && so[o] === 1)
        t === -1 && (t = n);
      else if (n !== 0 && (o === 32 || o === 9))
        s === -1 && t !== -1 && (s = n);
      else if (o === 44) {
        if (t === -1)
          throw new SyntaxError(`Unexpected character at index ${n}`);
        s === -1 && (s = n);
        let a = r.slice(t, s);
        if (e.has(a))
          throw new SyntaxError(`The "${a}" subprotocol is duplicated`);
        e.add(a), t = s = -1;
      } else
        throw new SyntaxError(`Unexpected character at index ${n}`);
    }
    if (t === -1 || s !== -1)
      throw new SyntaxError("Unexpected end of input");
    let i = r.slice(t, n);
    if (e.has(i))
      throw new SyntaxError(`The "${i}" subprotocol is duplicated`);
    return e.add(i), e;
  }
  ks.exports = { parse: no };
});
var Ls = b((Ol, Rs) => {
  var io = v("events"), Ye = v("http"), { Duplex: Al } = v("stream"), { createHash: oo } = v("crypto"), Cs = Ft(), J = xe(), ao = Ts(), lo = Ve(), { CLOSE_TIMEOUT: co, GUID: ho, kWebSocket: fo } = N(), uo = /^[+/0-9A-Za-z]{22}==$/, Ps = 0, As = 1, Us = 2, qt = class extends io {
    constructor(e, t) {
      if (super(), e = { allowSynchronousEvents: true, autoPong: true, maxPayload: 100 * 1024 * 1024, skipUTF8Validation: false, perMessageDeflate: false, handleProtocols: null, clientTracking: true, closeTimeout: co, verifyClient: null, noServer: false, backlog: null, server: null, host: null, path: null, port: null, WebSocket: lo, ...e }, e.port == null && !e.server && !e.noServer || e.port != null && (e.server || e.noServer) || e.server && e.noServer)
        throw new TypeError('One and only one of the "port", "server", or "noServer" options must be specified');
      if (e.port != null ? (this._server = Ye.createServer((s, n) => {
        let i = Ye.STATUS_CODES[426];
        n.writeHead(426, { "Content-Length": i.length, "Content-Type": "text/plain" }), n.end(i);
      }), this._server.listen(e.port, e.host, e.backlog, t)) : e.server && (this._server = e.server), this._server) {
        let s = this.emit.bind(this, "connection");
        this._removeListeners = po(this._server, { listening: this.emit.bind(this, "listening"), error: this.emit.bind(this, "error"), upgrade: (n, i, o) => {
          this.handleUpgrade(n, i, o, s);
        } });
      }
      e.perMessageDeflate === true && (e.perMessageDeflate = {}), e.clientTracking && (this.clients = new Set, this._shouldEmitClose = false), this.options = e, this._state = Ps;
    }
    address() {
      if (this.options.noServer)
        throw new Error('The server is operating in "noServer" mode');
      return this._server ? this._server.address() : null;
    }
    close(e) {
      if (this._state === Us) {
        e && this.once("close", () => {
          e(new Error("The server is not running"));
        }), process.nextTick(Te, this);
        return;
      }
      if (e && this.once("close", e), this._state !== As)
        if (this._state = As, this.options.noServer || this.options.server)
          this._server && (this._removeListeners(), this._removeListeners = this._server = null), this.clients ? this.clients.size ? this._shouldEmitClose = true : process.nextTick(Te, this) : process.nextTick(Te, this);
        else {
          let t = this._server;
          this._removeListeners(), this._removeListeners = this._server = null, t.close(() => {
            Te(this);
          });
        }
    }
    shouldHandle(e) {
      if (this.options.path) {
        let t = e.url.indexOf("?");
        if ((t !== -1 ? e.url.slice(0, t) : e.url) !== this.options.path)
          return false;
      }
      return true;
    }
    handleUpgrade(e, t, s, n) {
      t.on("error", Os);
      let i = e.headers["sec-websocket-key"], o = e.headers.upgrade, a = +e.headers["sec-websocket-version"];
      if (e.method !== "GET") {
        Q(this, e, t, 405, "Invalid HTTP method");
        return;
      }
      if (o === undefined || o.toLowerCase() !== "websocket") {
        Q(this, e, t, 400, "Invalid Upgrade header");
        return;
      }
      if (i === undefined || !uo.test(i)) {
        Q(this, e, t, 400, "Missing or invalid Sec-WebSocket-Key header");
        return;
      }
      if (a !== 13 && a !== 8) {
        Q(this, e, t, 400, "Missing or invalid Sec-WebSocket-Version header", { "Sec-WebSocket-Version": "13, 8" });
        return;
      }
      if (!this.shouldHandle(e)) {
        Ce(t, 400);
        return;
      }
      let l = e.headers["sec-websocket-protocol"], c = new Set;
      if (l !== undefined)
        try {
          c = ao.parse(l);
        } catch {
          Q(this, e, t, 400, "Invalid Sec-WebSocket-Protocol header");
          return;
        }
      let h = e.headers["sec-websocket-extensions"], f = {};
      if (this.options.perMessageDeflate && h !== undefined) {
        let d = new J(this.options.perMessageDeflate, true, this.options.maxPayload);
        try {
          let _ = Cs.parse(h);
          _[J.extensionName] && (d.accept(_[J.extensionName]), f[J.extensionName] = d);
        } catch {
          Q(this, e, t, 400, "Invalid or unacceptable Sec-WebSocket-Extensions header");
          return;
        }
      }
      if (this.options.verifyClient) {
        let d = { origin: e.headers[`${a === 8 ? "sec-websocket-origin" : "origin"}`], secure: !!(e.socket.authorized || e.socket.encrypted), req: e };
        if (this.options.verifyClient.length === 2) {
          this.options.verifyClient(d, (_, y, p, S) => {
            if (!_)
              return Ce(t, y || 401, p, S);
            this.completeUpgrade(f, i, c, e, t, s, n);
          });
          return;
        }
        if (!this.options.verifyClient(d))
          return Ce(t, 401);
      }
      this.completeUpgrade(f, i, c, e, t, s, n);
    }
    completeUpgrade(e, t, s, n, i, o, a) {
      if (!i.readable || !i.writable)
        return i.destroy();
      if (i[fo])
        throw new Error("server.handleUpgrade() was called more than once with the same socket, possibly due to a misconfiguration");
      if (this._state > Ps)
        return Ce(i, 503);
      let c = ["HTTP/1.1 101 Switching Protocols", "Upgrade: websocket", "Connection: Upgrade", `Sec-WebSocket-Accept: ${oo("sha1").update(t + ho).digest("base64")}`], h = new this.options.WebSocket(null, undefined, this.options);
      if (s.size) {
        let f = this.options.handleProtocols ? this.options.handleProtocols(s, n) : s.values().next().value;
        f && (c.push(`Sec-WebSocket-Protocol: ${f}`), h._protocol = f);
      }
      if (e[J.extensionName]) {
        let f = e[J.extensionName].params, d = Cs.format({ [J.extensionName]: [f] });
        c.push(`Sec-WebSocket-Extensions: ${d}`), h._extensions = e;
      }
      this.emit("headers", c, n), i.write(c.concat(`\r
`).join(`\r
`)), i.removeListener("error", Os), h.setSocket(i, o, { allowSynchronousEvents: this.options.allowSynchronousEvents, maxPayload: this.options.maxPayload, skipUTF8Validation: this.options.skipUTF8Validation }), this.clients && (this.clients.add(h), h.on("close", () => {
        this.clients.delete(h), this._shouldEmitClose && !this.clients.size && process.nextTick(Te, this);
      })), a(h, n);
    }
  };
  Rs.exports = qt;
  function po(r, e) {
    for (let t of Object.keys(e))
      r.on(t, e[t]);
    return function() {
      for (let s of Object.keys(e))
        r.removeListener(s, e[s]);
    };
  }
  function Te(r) {
    r._state = Us, r.emit("close");
  }
  function Os() {
    this.destroy();
  }
  function Ce(r, e, t, s) {
    t = t || Ye.STATUS_CODES[e], s = { Connection: "close", "Content-Type": "text/html", "Content-Length": Buffer.byteLength(t), ...s }, r.once("finish", r.destroy), r.end(`HTTP/1.1 ${e} ${Ye.STATUS_CODES[e]}\r
` + Object.keys(s).map((n) => `${n}: ${s[n]}`).join(`\r
`) + `\r
\r
` + t);
  }
  function Q(r, e, t, s, n, i) {
    if (r.listenerCount("wsClientError")) {
      let o = new Error(n);
      Error.captureStackTrace(o, Q), r.emit("wsClientError", o, t, e);
    } else
      Ce(t, s, n, i);
  }
});
function Yt() {
  let r = process.env.XDG_CONFIG_HOME;
  return r ? Je.join(r, "poke") : Je.join(zs.homedir(), ".config", "poke");
}
function Qe() {
  return Je.join(Yt(), "credentials.json");
}
function Kt(r) {
  let e = Yt(), t = Qe();
  ye.mkdirSync(e, { recursive: true });
  let s = { token: r };
  ye.writeFileSync(t, JSON.stringify(s, null, 2));
  try {
    ye.chmodSync(t, 384);
  } catch {}
}
function U() {
  try {
    let r = ye.readFileSync(Qe(), "utf-8");
    return JSON.parse(r);
  } catch {
    return null;
  }
}
function Ae() {
  try {
    ye.unlinkSync(Qe());
  } catch {}
}
var et = class {
  apiKey;
  baseUrl;
  constructor(e) {
    let t = e?.apiKey ?? process.env.POKE_API_KEY ?? U()?.token;
    if (!t)
      throw new Error(["Missing API key. Find yours at https://poke.com/kitchen/api-keys", "", "Provide it in one of three ways:", '  1. new Poke({ apiKey: "pk_..." })', "  2. Set the POKE_API_KEY environment variable", "  3. Run `poke login` in your terminal"].join(`
`));
    this.apiKey = t, this.baseUrl = e?.baseUrl ?? process.env.POKE_API ?? "https://poke.com/api/v1";
  }
  async request({ path: e, body: t }) {
    let s = await fetch(`${this.baseUrl}${e}`, { method: "POST", headers: { Authorization: `Bearer ${this.apiKey}`, "Content-Type": "application/json" }, body: JSON.stringify(t) });
    if (!s.ok) {
      let n = await s.text(), i = "";
      try {
        let o = JSON.parse(n);
        i = o.error ?? o.message ?? "";
      } catch {}
      throw s.status === 401 ? new Error("Poke: Invalid API key. Get a new one at https://poke.com/kitchen/api-keys") : s.status === 403 ? new Error("Poke: API key doesn't have permission for this action. Check your key scopes at https://poke.com/kitchen/api-keys") : s.status === 429 ? new Error("Poke: Rate limited. Please slow down and retry.") : new Error(`Poke API error (${s.status}): ${i || s.statusText}`);
    }
    return s.json();
  }
  async sendMessage(e) {
    return this.request({ path: "/inbound/api-message", body: { message: e } });
  }
  async sendWebhook({ webhookUrl: e, webhookToken: t, data: s }) {
    let n = await fetch(e, { method: "POST", headers: { Authorization: `Bearer ${t}`, "Content-Type": "application/json" }, body: JSON.stringify(s) });
    if (!n.ok) {
      let i = await n.text(), o = "";
      try {
        let a = JSON.parse(i);
        o = a.error ?? a.message ?? "";
      } catch {}
      throw new Error(`Poke webhook error (${n.status}): ${o || n.statusText}`);
    }
    return n.json();
  }
  async createWebhook({ condition: e, action: t }) {
    return this.request({ path: "/api-keys/webhook", body: { condition: e, action: t } });
  }
};
var k = class extends Error {
  constructor(e) {
    super(e), this.name = "PokeAuthError";
  }
};
var Vs = "https://poke.com/api/v1";
async function Xt({ path: r, options: e = {}, token: t, baseUrl: s }) {
  let n = t ?? U()?.token;
  if (!n)
    throw new k("Not logged in. Run 'poke login'.");
  let i = s ?? process.env.POKE_API ?? Vs, o = new Headers(e.headers);
  o.set("Authorization", `Bearer ${n}`);
  let a = await fetch(`${i}${r}`, { ...e, headers: o });
  if (a.status === 401)
    throw Ae(), new k("Session expired. Run 'poke login' again.");
  return a;
}
var An = "https://poke.com/api/v1";
var On = "https://poke.com";
var Un = 5 * 60 * 1000;
var Rn = 2000;
function Ln(r) {
  return new Promise((e) => setTimeout(e, r));
}
async function Nn(r) {
  let e = U();
  if (e?.token)
    return { token: e.token };
  let t = r?.baseUrl ?? process.env.POKE_API ?? An, s = r?.frontendUrl ?? process.env.POKE_FRONTEND ?? On, n = r?.openBrowser ?? true, i = r?.timeoutMs ?? Un, o = await fetch(`${t}/cli-auth/code`, { method: "POST" });
  if (!o.ok)
    throw new k("Failed to create login code");
  let { deviceCode: a, userCode: l } = await o.json(), c = `${s}/device?code=${encodeURIComponent(l)}`;
  if (r?.onCode?.({ userCode: l, loginUrl: c }), n)
    try {
      let { default: f } = await Promise.resolve().then(() => (vr(), _r));
      await f(c);
    } catch {}
  let h = Date.now() + i;
  for (;Date.now() < h; ) {
    await Ln(Rn);
    let d = await (await fetch(`${t}/cli-auth/poll/${a}`)).json();
    if (d.status === "authenticated")
      return Kt(d.token), { token: d.token };
    if (d.status === "expired")
      throw new k("Login code expired.");
    if (d.status === "invalid")
      throw new k("Invalid login code.");
  }
  throw new k("Login timed out.");
}
function Wn() {
  return U()?.token != null;
}
function Bn() {
  return U()?.token ?? undefined;
}
var m = { Data: 0, WindowUpdate: 1, Ping: 2, GoAway: 3 };
var w = { SYN: 1, ACK: 2, FIN: 4, RST: 8 };
var G = { Normal: 0, ProtocolError: 1, InternalError: 2 };
var dt = 256 * 1024;
function pt() {
  return { acceptBacklog: 256, enableKeepAlive: true, keepAliveInterval: 30000, connectionWriteTimeout: 1e4, maxStreamWindowSize: dt, maxIncomingStreams: 1000 };
}
var A = class extends Error {
  constructor(e) {
    super(e), this.name = "YamuxError";
  }
};
var L = class extends A {
  constructor() {
    super("session shutdown"), this.name = "SessionShutdownError";
  }
};
var H = class extends A {
  constructor(e) {
    super(e ?? "stream closed"), this.name = "StreamClosedError";
  }
};
var ne = class extends A {
  constructor() {
    super("stream reset"), this.name = "StreamResetError";
  }
};
var ie = class extends A {
  constructor(e) {
    super(`received goaway: code ${e}`), this.name = "GoAwayError";
  }
};
function mt(r) {
  let e = new Uint8Array(12), t = new DataView(e.buffer);
  return t.setUint8(0, r.version), t.setUint8(1, r.type), t.setUint16(2, r.flags), t.setUint32(4, r.streamId), t.setUint32(8, r.length), e;
}
function gt(r) {
  if (r.length < 12)
    throw new Error("buffer too small for header");
  let e = new DataView(r.buffer, r.byteOffset, r.byteLength);
  return { version: e.getUint8(0), type: e.getUint8(1), flags: e.getUint16(2), streamId: e.getUint32(4), length: e.getUint32(8) };
}
var Le = class {
  buffer = [];
  maxSize;
  waitingPushers = [];
  waitingPoppers = [];
  isClosed = false;
  constructor(e = 1 / 0) {
    this.maxSize = e;
  }
  get length() {
    return this.buffer.length;
  }
  get closed() {
    return this.isClosed;
  }
  push(e) {
    return this.isClosed ? Promise.reject(new Error("queue is closed")) : this.waitingPoppers.length > 0 ? (this.waitingPoppers.shift().resolve(e), Promise.resolve()) : this.buffer.length < this.maxSize ? (this.buffer.push(e), Promise.resolve()) : new Promise((t, s) => {
      this.waitingPushers.push({ value: e, resolve: t, reject: s });
    });
  }
  tryPush(e) {
    return this.isClosed ? false : this.waitingPoppers.length > 0 ? (this.waitingPoppers.shift().resolve(e), true) : this.buffer.length < this.maxSize ? (this.buffer.push(e), true) : false;
  }
  pop() {
    if (this.buffer.length > 0) {
      let e = this.buffer.shift();
      if (this.waitingPushers.length > 0) {
        let t = this.waitingPushers.shift();
        this.buffer.push(t.value), t.resolve();
      }
      return Promise.resolve(e);
    }
    if (this.waitingPushers.length > 0) {
      let e = this.waitingPushers.shift();
      return e.resolve(), Promise.resolve(e.value);
    }
    return this.isClosed ? Promise.reject(new Error("queue is closed")) : new Promise((e, t) => {
      this.waitingPoppers.push({ resolve: e, reject: t });
    });
  }
  close() {
    this.isClosed = true;
    let e = new Error("queue is closed");
    for (let t of this.waitingPoppers)
      t.reject(e);
    this.waitingPoppers = [];
    for (let t of this.waitingPushers)
      t.reject(e);
    this.waitingPushers = [];
  }
};
var V = class {
  promise;
  resolve;
  reject;
  settled = false;
  constructor() {
    this.promise = new Promise((e, t) => {
      this.resolve = (s) => {
        this.settled = true, e(s);
      }, this.reject = (s) => {
        this.settled = true, t(s);
      };
    });
  }
};
var Ne = class {
  buf;
  head = 0;
  tail = 0;
  count = 0;
  constructor(e) {
    this.buf = new Uint8Array(e);
  }
  get length() {
    return this.count;
  }
  get capacity() {
    return this.buf.length;
  }
  get available() {
    return this.buf.length - this.count;
  }
  write(e) {
    let t = Math.min(e.length, this.available);
    if (t === 0)
      return 0;
    let s = Math.min(t, this.buf.length - this.tail);
    return this.buf.set(e.subarray(0, s), this.tail), s < t && this.buf.set(e.subarray(s, t), 0), this.tail = (this.tail + t) % this.buf.length, this.count += t, t;
  }
  read(e) {
    let t = Math.min(e.length, this.count);
    if (t === 0)
      return 0;
    let s = Math.min(t, this.buf.length - this.head);
    return e.set(this.buf.subarray(this.head, this.head + s)), s < t && e.set(this.buf.subarray(0, t - s), s), this.head = (this.head + t) % this.buf.length, this.count -= t, t;
  }
  reset() {
    this.head = 0, this.tail = 0, this.count = 0;
  }
};
var u = { Init: 0, SYNSent: 1, SYNReceived: 2, Established: 3, LocalClose: 4, RemoteClose: 5, Closed: 6, Reset: 7 };
var oe = class {
  id;
  session;
  state;
  recvBuf;
  recvWindow;
  sendWindow;
  readNotify = null;
  sendWindowNotify = null;
  maxWindow;
  constructor(e, t, s, n = u.Init) {
    this.id = e, this.session = t, this.state = n, this.maxWindow = s, this.recvBuf = new Ne(s), this.recvWindow = s, this.sendWindow = s;
  }
  getState() {
    return this.state;
  }
  setState(e) {
    this.state = e;
  }
  async read(e) {
    for (;; ) {
      if (this.state === u.Reset)
        throw new ne;
      let t = this.recvBuf.read(e);
      if (t > 0)
        return this.maybeSendWindowUpdate(), t;
      if (this.state === u.RemoteClose || this.state === u.Closed)
        return 0;
      if (this.state !== u.Established && this.state !== u.LocalClose)
        throw new H;
      this.readNotify = new V, await this.readNotify.promise;
    }
  }
  async write(e) {
    let t = 0;
    for (;t < e.length; ) {
      for (this.checkWritable();this.sendWindow === 0; ) {
        this.checkWritable(), this.sendWindowNotify = new V;
        let i = this.session.getWriteTimeout(), o = setTimeout(() => {
          this.sendWindowNotify?.reject(new H("write timeout: peer not sending window updates"));
        }, i);
        try {
          await this.sendWindowNotify.promise;
        } finally {
          clearTimeout(o);
        }
        this.checkWritable();
      }
      let s = Math.min(e.length - t, this.sendWindow), n = e.subarray(t, t + s);
      await this.session.sendFrame(m.Data, 0, this.id, s, n), this.sendWindow -= s, t += s;
    }
  }
  async close() {
    switch (this.state) {
      case u.Established:
        this.state = u.LocalClose;
        break;
      case u.RemoteClose:
        this.state = u.Closed;
        break;
      default:
        return;
    }
    await this.session.sendFrame(m.WindowUpdate, w.FIN, this.id, 0), this.state === u.Closed && this.session.removeStream(this.id), this.wakeRead();
  }
  reset() {
    this.state === u.Closed || this.state === u.Reset || (this.state = u.Reset, this.session.sendFrameNoWait(m.WindowUpdate, w.RST, this.id, 0), this.session.removeStream(this.id), this.wakeRead(), this.wakeSendWindow());
  }
  forceClose() {
    this.state === u.Closed || this.state === u.Reset || (this.recvBuf.length > 0 ? this.state = u.RemoteClose : this.state = u.Reset, this.wakeRead(), this.wakeSendWindow());
  }
  deliverData(e) {
    return this.state === u.RemoteClose || this.state === u.Closed || this.state === u.Reset || e.length > this.recvWindow || this.recvBuf.write(e) < e.length ? false : (this.recvWindow -= e.length, this.wakeRead(), true);
  }
  processFlags(e) {
    if (e & w.RST) {
      this.state = u.Reset, this.wakeRead(), this.wakeSendWindow(), this.session.removeStream(this.id);
      return;
    }
    if (e & w.FIN) {
      switch (this.state) {
        case u.Established:
        case u.SYNSent:
        case u.SYNReceived:
          this.state = u.RemoteClose;
          break;
        case u.LocalClose:
          this.state = u.Closed, this.session.removeStream(this.id);
          break;
      }
      this.wakeRead();
    }
    e & w.ACK && this.state === u.SYNSent && (this.state = u.Established);
  }
  updateSendWindow(e) {
    this.sendWindow = Math.min(this.sendWindow + e, 4294967295), this.wakeSendWindow();
  }
  checkWritable() {
    if (this.state === u.Reset)
      throw new ne;
    if (this.state !== u.Established && this.state !== u.RemoteClose)
      throw new H;
  }
  maybeSendWindowUpdate() {
    let e = this.maxWindow, t = e - this.recvWindow - this.recvBuf.length;
    t < e / 2 || (this.recvWindow += t, this.session.sendFrameNoWait(m.WindowUpdate, 0, this.id, t));
  }
  wakeRead() {
    if (this.readNotify) {
      let e = this.readNotify;
      this.readNotify = null, e.resolve();
    }
  }
  wakeSendWindow() {
    if (this.sendWindowNotify) {
      let e = this.sendWindowNotify;
      this.sendWindowNotify = null, e.resolve();
    }
  }
};
var Ie = class {
  transport;
  config;
  isClient;
  nextStreamId;
  streams = new Map;
  acceptQueue;
  pings = new Map;
  nextPingId = 0;
  closed = false;
  remoteGoAway = false;
  shutdownErr = null;
  highestInboundStreamId = 0;
  writeMutex = Promise.resolve();
  recvLoopDone;
  keepaliveTimer = null;
  constructor(e, t, s) {
    this.transport = e, this.config = { ...pt(), ...s }, this.isClient = t, this.nextStreamId = t ? 1 : 2, this.acceptQueue = new Le(this.config.acceptBacklog), this.recvLoopDone = this.recvLoop().catch(() => {}), this.config.enableKeepAlive && (this.keepaliveTimer = setInterval(() => {
      this.ping().catch(() => {
        this.closeWithError(new A("keepalive timeout"));
      });
    }, this.config.keepAliveInterval));
  }
  async open() {
    if (this.closed)
      throw new L;
    if (this.remoteGoAway)
      throw new ie(0);
    let e = this.nextStreamId;
    this.nextStreamId += 2;
    let t = new oe(e, this, this.config.maxStreamWindowSize, u.Init);
    this.streams.set(e, t);
    try {
      await this.sendFrame(m.WindowUpdate, w.SYN, e, 0);
    } catch (s) {
      throw t.forceClose(), this.streams.delete(e), s;
    }
    return t.setState(u.Established), t;
  }
  async accept() {
    if (this.closed)
      throw new L;
    let e = await this.acceptQueue.pop();
    try {
      await this.sendFrame(m.WindowUpdate, w.ACK, e.id, 0);
    } catch (t) {
      throw e.forceClose(), this.streams.delete(e.id), t;
    }
    return e.setState(u.Established), e;
  }
  async ping() {
    if (this.closed)
      throw new L;
    let e = this.nextPingId++, t = new V;
    this.pings.set(e, t);
    let s = Date.now();
    await this.sendFrame(m.Ping, w.SYN, 0, e);
    let n = setTimeout(() => {
      this.pings.has(e) && (this.pings.delete(e), t.reject(new A("ping timeout")));
    }, this.config.connectionWriteTimeout);
    try {
      return await t.promise, Date.now() - s;
    } finally {
      clearTimeout(n);
    }
  }
  async close() {
    if (!this.closed) {
      this.keepaliveTimer && (clearInterval(this.keepaliveTimer), this.keepaliveTimer = null);
      try {
        await this.sendFrame(m.GoAway, 0, 0, G.Normal);
      } catch {}
      this.closed = true, this.shutdownErr = new L;
      for (let e of this.streams.values())
        e.forceClose();
      this.streams.clear(), this.acceptQueue.close();
      for (let [, e] of this.pings)
        e.reject(new L);
      this.pings.clear();
      try {
        await this.transport.close();
      } catch {}
      await this.recvLoopDone;
    }
  }
  isClosed() {
    return this.closed;
  }
  numStreams() {
    return this.streams.size;
  }
  getMaxStreamWindowSize() {
    return this.config.maxStreamWindowSize;
  }
  getWriteTimeout() {
    return this.config.connectionWriteTimeout;
  }
  async sendFrame(e, t, s, n, i) {
    if (this.closed && this.shutdownErr)
      throw this.shutdownErr;
    let o = mt({ version: 0, type: e, flags: t, streamId: s, length: n }), a;
    i && i.length > 0 ? (a = new Uint8Array(12 + i.length), a.set(o), a.set(i, 12)) : a = o;
    let l, c = this.writeMutex;
    this.writeMutex = new Promise((h) => {
      l = h;
    }), await c;
    try {
      await this.transport.write(a);
    } finally {
      l();
    }
  }
  sendFrameNoWait(e, t, s, n) {
    this.sendFrame(e, t, s, n).catch(() => {});
  }
  removeStream(e) {
    this.streams.delete(e);
  }
  async recvLoop() {
    try {
      for (;!this.closed; ) {
        let e = await this.transport.read(12), t = gt(e);
        if (t.version !== 0) {
          await this.goAway(G.ProtocolError);
          return;
        }
        switch (t.type) {
          case m.Ping:
            this.handlePing(t.flags, t.length);
            break;
          case m.GoAway:
            this.handleGoAway(t.length);
            break;
          case m.Data:
          case m.WindowUpdate:
            await this.handleStreamMessage(t);
            break;
          default:
            await this.goAway(G.ProtocolError);
            return;
        }
      }
    } catch (e) {
      this.closed || this.closeWithError(e instanceof Error ? e : new Error(String(e)));
    }
  }
  handlePing(e, t) {
    if (e & w.SYN)
      this.sendFrameNoWait(m.Ping, w.ACK, 0, t);
    else if (e & w.ACK) {
      let s = this.pings.get(t);
      s && (this.pings.delete(t), s.resolve(t));
    }
  }
  handleGoAway(e) {
    this.remoteGoAway = true, this.acceptQueue.close(), e !== G.Normal && this.closeWithError(new ie(e));
  }
  async handleStreamMessage(e) {
    if (e.type === m.Data && e.length > this.config.maxStreamWindowSize) {
      await this.goAway(G.ProtocolError);
      return;
    }
    e.flags & w.SYN && this.incomingStream(e.streamId);
    let t = this.streams.get(e.streamId);
    if (!t) {
      e.type === m.Data && e.length > 0 && await this.transport.read(e.length), e.flags & w.RST || this.sendFrameNoWait(m.WindowUpdate, w.RST, e.streamId, 0);
      return;
    }
    let s = e.flags & ~w.SYN;
    switch (e.type) {
      case m.Data: {
        if (e.length > 0) {
          let n = await this.transport.read(e.length);
          if (!t.deliverData(n)) {
            t.reset();
            return;
          }
        }
        s && t.processFlags(s);
        break;
      }
      case m.WindowUpdate: {
        e.length > 0 && t.updateSendWindow(e.length), s && t.processFlags(s);
        break;
      }
    }
  }
  incomingStream(e) {
    if (e === 0) {
      this.sendFrameNoWait(m.WindowUpdate, w.RST, e, 0);
      return;
    }
    if (this.streams.has(e))
      return;
    let t = this.isClient;
    if (e % 2 === 0 !== t) {
      this.sendFrameNoWait(m.WindowUpdate, w.RST, e, 0);
      return;
    }
    if (e <= this.highestInboundStreamId) {
      this.sendFrameNoWait(m.WindowUpdate, w.RST, e, 0);
      return;
    }
    if (this.highestInboundStreamId = e, this.streams.size >= this.config.maxIncomingStreams) {
      this.sendFrameNoWait(m.WindowUpdate, w.RST, e, 0);
      return;
    }
    let s = new oe(e, this, this.config.maxStreamWindowSize, u.SYNReceived);
    this.streams.set(e, s), this.acceptQueue.tryPush(s) || (this.streams.delete(e), this.sendFrameNoWait(m.WindowUpdate, w.RST, e, 0));
  }
  async goAway(e) {
    try {
      await this.sendFrame(m.GoAway, 0, 0, e);
    } catch {}
    this.closeWithError(new A(`protocol error: goaway ${e}`));
  }
  closeWithError(e) {
    if (!this.closed) {
      this.closed = true, this.shutdownErr = e, this.keepaliveTimer && (clearInterval(this.keepaliveTimer), this.keepaliveTimer = null);
      for (let t of this.streams.values())
        t.forceClose();
      this.streams.clear(), this.acceptQueue.close();
      for (let [, t] of this.pings)
        t.reject(e);
      this.pings.clear(), this.transport.close().catch(() => {});
    }
  }
};
function yt(r, e) {
  return new Ie(r, true, e);
}
var wt = class {
  chunks = [];
  totalBytes = 0;
  pendingRead = null;
  closed = false;
  closeError = null;
  push(e) {
    if (!this.closed) {
      if (this.chunks.push(new Uint8Array(e)), this.totalBytes += e.length, this.totalBytes > 16777216) {
        this.error(new Error("transport buffer overflow"));
        return;
      }
      this.tryFulfill();
    }
  }
  error(e) {
    if (this.closeError = e, this.closed = true, this.pendingRead) {
      let { reject: t } = this.pendingRead;
      this.pendingRead = null, t(e);
    }
  }
  end() {
    if (this.closed = true, this.pendingRead) {
      let { reject: e } = this.pendingRead;
      this.pendingRead = null, e(this.closeError ?? new Error("transport closed"));
    }
  }
  read(e) {
    return this.totalBytes >= e ? Promise.resolve(this.consume(e)) : this.closed ? Promise.reject(this.closeError ?? new Error("transport closed")) : new Promise((t, s) => {
      this.pendingRead = { bytes: e, resolve: t, reject: s };
    });
  }
  tryFulfill() {
    if (this.pendingRead && this.totalBytes >= this.pendingRead.bytes) {
      let { bytes: e, resolve: t } = this.pendingRead;
      this.pendingRead = null, t(this.consume(e));
    }
  }
  consume(e) {
    if (e === 0)
      return new Uint8Array(0);
    let t = new Uint8Array(e), s = 0;
    for (;s < e; ) {
      let n = this.chunks[0], i = e - s;
      n.length <= i ? (t.set(n, s), s += n.length, this.totalBytes -= n.length, this.chunks.shift()) : (t.set(n.subarray(0, i), s), this.chunks[0] = n.subarray(i), this.totalBytes -= i, s = e);
    }
    return t;
  }
};
function _t(r) {
  let e = new wt;
  return r.on("message", (t) => {
    t instanceof ArrayBuffer ? e.push(new Uint8Array(t)) : e.push(t instanceof Uint8Array ? t : new Uint8Array(t));
  }), r.on("error", (t) => e.error(t)), r.on("close", () => e.end()), { read(t) {
    return e.read(t);
  }, write(t) {
    return new Promise((s, n) => {
      r.send(t, (i) => {
        i ? n(i) : s();
      });
    });
  }, async close() {
    r.close();
  } };
}
var vt = 64 * 1024;
var xt = 64 * 1024 * 1024;
var xr = new TextDecoder;
var br = new TextEncoder;
function Mn(r, e) {
  for (let t = 0;t <= e - 4; t++)
    if (r[t] === 13 && r[t + 1] === 10 && r[t + 2] === 13 && r[t + 3] === 10)
      return t;
  return -1;
}
async function Dn(r) {
  let e = new Uint8Array(4096), t = 0;
  for (;t < vt; ) {
    if (t === e.length) {
      let o = new Uint8Array(Math.min(e.length * 2, vt));
      o.set(e), e = o;
    }
    let s = new Uint8Array(Math.min(4096, e.length - t)), n = await r.read(s);
    if (n === 0)
      throw new Error("stream closed before headers complete");
    e.set(s.subarray(0, n), t), t += n;
    let i = Mn(e, t);
    if (i >= 0) {
      let o = i + 4;
      return { headerBytes: e.subarray(0, o), remainder: e.subarray(o, t) };
    }
  }
  throw new Error("headers too large");
}
function $n(r) {
  let e = r.split(`\r
`), t = e[0];
  if (!t)
    throw new Error("empty request");
  let s = t.split(" ");
  if (s.length < 3)
    throw new Error("invalid request line");
  let n = s[0], i = s[1];
  if (!i.startsWith("/"))
    throw new Error("invalid request path");
  let o = s[2], a = {};
  for (let l = 1;l < e.length; l++) {
    let c = e[l];
    if (!c)
      break;
    let h = c.indexOf(":");
    if (h === -1)
      continue;
    let f = c.substring(0, h).trim().toLowerCase(), d = c.substring(h + 1).trim();
    a[f] = d;
  }
  return { method: n, path: i, httpVersion: o, headers: a, body: null };
}
async function jn(r, e, t) {
  let s = e["content-length"];
  if (s) {
    let i = parseInt(s, 10);
    if (isNaN(i) || i <= 0)
      return null;
    if (i > xt)
      throw new Error(`body too large: ${i} bytes`);
    let o = new Uint8Array(i), a = 0;
    if (t.length > 0) {
      let l = Math.min(t.length, i);
      o.set(t.subarray(0, l)), a = l;
    }
    for (;a < i; ) {
      let l = new Uint8Array(Math.min(i - a, 32768)), c = await r.read(l);
      if (c === 0)
        break;
      o.set(l.subarray(0, c), a), a += c;
    }
    return o.subarray(0, a);
  }
  let n = e["transfer-encoding"];
  return n && n.toLowerCase().includes("chunked") ? qn(r, t) : t.length > 0 ? t : null;
}
async function qn(r, e) {
  let t = [], s = e, n = 0;
  for (;; ) {
    for (;!Gn(s); ) {
      if (s.length > vt)
        throw new Error("chunked size line too large");
      let l = new Uint8Array(4096), c = await r.read(l);
      if (c === 0)
        return Sr(t);
      s = St(s, l.subarray(0, c));
    }
    let i = Er(s), o = xr.decode(s.subarray(0, i)), a = parseInt(o.trim(), 16);
    if (s = s.subarray(i + 2), isNaN(a) || a <= 0)
      break;
    if (n += a, n > xt)
      throw new Error(`chunked body too large: ${n} bytes`);
    for (;s.length < a + 2; ) {
      let l = new Uint8Array(Math.max(4096, a - s.length)), c = await r.read(l);
      if (c === 0)
        break;
      s = St(s, l.subarray(0, c));
    }
    t.push(s.subarray(0, a)), s = s.subarray(a + 2);
  }
  return Sr(t);
}
function Gn(r) {
  return Er(r) >= 0;
}
function Er(r) {
  for (let e = 0;e < r.length - 1; e++)
    if (r[e] === 13 && r[e + 1] === 10)
      return e;
  return -1;
}
function St(r, e) {
  let t = new Uint8Array(r.length + e.length);
  return t.set(r), t.set(e, r.length), t;
}
function Sr(r) {
  let e = r.reduce((n, i) => n + i.length, 0), t = new Uint8Array(e), s = 0;
  for (let n of r)
    t.set(n, s), s += n.length;
  return t;
}
function _e(r) {
  return r.replace(/[\r\n]/g, " ");
}
function kr(r, e, t, s) {
  let n = `HTTP/1.1 ${r} ${_e(e)}\r
`;
  for (let [o, a] of Object.entries(t))
    if (Array.isArray(a))
      for (let l of a)
        n += `${_e(o)}: ${_e(l)}\r
`;
    else
      n += `${_e(o)}: ${_e(a)}\r
`;
  !t["content-length"] && !t["Content-Length"] && (n += `Content-Length: ${s.length}\r
`), n += `\r
`;
  let i = br.encode(n);
  return St(i, s);
}
async function Tr(r, e, t) {
  try {
    let { headerBytes: s, remainder: n } = await Dn(r), i = xr.decode(s), o = $n(i);
    o.body = await jn(r, o.headers, n);
    let [a, l] = e.split(":"), c = l ? parseInt(l, 10) : 80, h = await Hn(o, a, c);
    await r.write(h);
  } catch (s) {
    t?.warn("proxy error", { error: String(s) });
    try {
      let n = br.encode("Bad Gateway"), i = kr(502, "Bad Gateway", {}, n);
      await r.write(i);
    } catch {}
  } finally {
    try {
      await r.close();
    } catch {}
  }
}
function Hn(r, e, t) {
  return new Promise((s, n) => {
    let i = {}, o = new Set(["host", "transfer-encoding", "connection", "keep-alive", "te", "trailer", "upgrade"]);
    for (let [l, c] of Object.entries(r.headers))
      o.has(l) || (i[l] = c);
    i.host = `${e}:${t}`, r.body && r.body.length > 0 && !i["content-length"] && (i["content-length"] = String(r.body.length));
    let a = Fn.request({ hostname: e, port: t, method: r.method, path: r.path, headers: i }, (l) => {
      let c = [], h = 0;
      l.on("data", (f) => {
        if (h += f.length, h > xt) {
          l.destroy(new Error("response body exceeds size limit"));
          return;
        }
        c.push(f);
      }), l.on("end", () => {
        let f = Buffer.concat(c), d = {}, _ = new Set(["transfer-encoding", "connection", "keep-alive", "te", "trailer", "upgrade"]);
        for (let [p, S] of Object.entries(l.headers))
          S !== undefined && !_.has(p) && (d[p] = S);
        d["content-length"] = String(f.length);
        let y = kr(l.statusCode ?? 500, l.statusMessage ?? "Internal Server Error", d, new Uint8Array(f));
        s(y);
      }), l.on("error", n);
    });
    a.on("error", n), r.body && r.body.length > 0 && a.write(r.body), a.end();
  });
}
var Y = class extends Error {
  constructor(e) {
    super(e), this.name = "PikoAuthError";
  }
};
var $ = class extends Error {
  statusCode;
  constructor(e, t) {
    super(e), this.name = "PikoConnectionError", this.statusCode = t;
  }
};
var Cr = 100;
var zn = 15000;
var Vn = 0.3;
var Yn = new Set([400, 401, 403, 404, 405, 410]);
function Pr(r) {
  if (r instanceof Y)
    return false;
  if (typeof r == "object" && r !== null && "statusCode" in r) {
    let e = r.statusCode;
    if (typeof e == "number")
      return !Yn.has(e);
  }
  if (r instanceof Error) {
    let e = r.message.toLowerCase();
    if (/\b401\b/.test(e) || /\b403\b/.test(e) || e.includes("unauthorized") || e.includes("forbidden"))
      return false;
  }
  return true;
}
function Ar(r) {
  let e = Math.min(Cr * Math.pow(2, r), zn), t = e * Vn * (Math.random() * 2 - 1);
  return Math.max(Cr, Math.round(e + t));
}
var mo = ge(Es(), 1);
var go = ge(Lt(), 1);
var yo = ge(Wt(), 1);
var Ns = ge(Ve(), 1);
var wo = ge(Ls(), 1);
var Is = Ns.default;
async function Ws(r) {
  let e = _o(r.upstreamUrl, r.endpointId);
  return new Promise((t, s) => {
    if (r.signal?.aborted) {
      s(new $("aborted"));
      return;
    }
    let n = new Is(e, { headers: { Authorization: `Bearer ${r.token}` } });
    n.binaryType = "nodebuffer";
    let i = () => {
      n.close(), s(new $("aborted"));
    };
    r.signal?.addEventListener("abort", i, { once: true }), n.on("open", () => {
      r.signal?.removeEventListener("abort", i);
      let o = _t(n);
      t({ transport: o, ws: n });
    }), n.on("error", (o) => {
      r.signal?.removeEventListener("abort", i), s(new $(o.message));
    }), n.on("unexpected-response", (o, a) => {
      r.signal?.removeEventListener("abort", i), a.statusCode === 401 || a.statusCode === 403 ? s(new Y(`Authentication failed: HTTP ${a.statusCode}`)) : s(new $(`Unexpected HTTP ${a.statusCode} from upstream`, a.statusCode));
    });
  });
}
function _o(r, e) {
  return `${r.replace(/\/$/, "").replace(/^https:\/\//, "wss://").replace(/^http:\/\//, "ws://")}/piko/v1/upstream/${e}`;
}
var vo = { info() {}, warn() {}, error() {} };
var Pe = class {
  opts;
  logger;
  abortController;
  session = null;
  running = false;
  isConnected = false;
  listeners = new Map;
  constructor(e) {
    this.opts = { ...e, logger: e.logger ?? vo }, this.logger = this.opts.logger, this.abortController = new AbortController, e.signal && e.signal.addEventListener("abort", () => {
      this.abortController.abort();
    });
  }
  get connected() {
    return this.isConnected;
  }
  on(e, t) {
    let s = this.listeners.get(e);
    s || (s = new Set, this.listeners.set(e, s)), s.add(t);
  }
  off(e, t) {
    this.listeners.get(e)?.delete(t);
  }
  async start() {
    this.running || (this.abortController.signal.aborted && (this.abortController = new AbortController), this.running = true, this.runLoop());
  }
  async stop() {
    this.running = false, this.abortController.abort(), this.session && (await this.session.close().catch(() => {}), this.session = null), this.setConnected(false);
  }
  async runLoop() {
    let e = 0;
    for (;this.running && !this.abortController.signal.aborted; )
      try {
        await this.connectAndServe(), e = 0;
      } catch (t) {
        if (this.setConnected(false), this.session = null, !this.running || this.abortController.signal.aborted)
          return;
        if (!Pr(t)) {
          this.logger.error("Fatal error, not retrying"), this.emit("error", t), this.running = false;
          return;
        }
        e++;
        let s = Ar(e);
        this.logger.warn("Connection lost, reconnecting", { attempt: e, retryInMs: s }), this.emit("disconnected"), await this.sleep(s);
      }
  }
  async connectAndServe() {
    let { transport: e } = await Ws({ upstreamUrl: this.opts.upstreamUrl, endpointId: this.opts.endpointId, token: this.opts.token, signal: this.abortController.signal }), t = yt(e, { enableKeepAlive: true, keepAliveInterval: 30000 });
    this.session = t, this.setConnected(true), this.emit("connected"), this.logger.info("Connected to upstream");
    try {
      for (;this.running && !t.isClosed(); ) {
        let s = await t.accept();
        Tr(s, this.opts.localAddr, { warn: this.logger.warn.bind(this.logger) }).catch((n) => {
          this.logger.warn("Stream proxy error", { error: String(n) });
        });
      }
    } finally {
      t.isClosed() || await t.close().catch(() => {});
    }
  }
  setConnected(e) {
    this.isConnected = e;
  }
  emit(e, ...t) {
    let s = this.listeners.get(e);
    if (s)
      for (let n of s)
        try {
          n(...t);
        } catch {}
  }
  sleep(e) {
    return new Promise((t, s) => {
      if (this.abortController.signal.aborted) {
        t();
        return;
      }
      let n = setTimeout(t, e);
      this.abortController.signal.addEventListener("abort", () => {
        clearTimeout(n), t();
      }, { once: true });
    });
  }
};
function Bs(r) {
  return new Pe(r);
}
var Gt = class {
  options;
  localUrl;
  listeners = new Map;
  pikoClient = null;
  syncTimer = null;
  connectionId = null;
  tunnelUrl = null;
  _connected = false;
  get info() {
    return !this.connectionId || !this.tunnelUrl ? null : { connectionId: this.connectionId, tunnelUrl: this.tunnelUrl, localUrl: this.options.url, name: this.options.name };
  }
  get connected() {
    return this._connected;
  }
  constructor(e) {
    this.options = e;
    try {
      this.localUrl = new URL(e.url);
    } catch {
      throw new Error(`Invalid URL: ${e.url}`);
    }
  }
  on(e, t) {
    return this.listeners.has(e) || this.listeners.set(e, new Set), this.listeners.get(e).add(t), this;
  }
  off(e, t) {
    return this.listeners.get(e)?.delete(t), this;
  }
  emit(e, ...t) {
    for (let s of this.listeners.get(e) ?? [])
      s(...t);
  }
  resolveToken() {
    let e = this.options.token ?? U()?.token;
    if (!e)
      throw new k("Not logged in. Run 'poke login'.");
    return e;
  }
  fetchAuth({ path: e, options: t }) {
    return Xt({ path: e, options: t, token: this.resolveToken(), baseUrl: this.options.baseUrl });
  }
  async start() {
    let e = { name: this.options.name, serverUrl: this.options.url, tunnel: true };
    this.options.clientId && (e.clientId = this.options.clientId), this.options.clientSecret && (e.clientSecret = this.options.clientSecret);
    let t = await this.fetchAuth({ path: "/mcp/connections/cli", options: { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(e) } });
    if (!t.ok) {
      let a = await t.text(), l = `HTTP ${t.status}`;
      try {
        l = JSON.parse(a).message ?? l;
      } catch {}
      throw new Error(`Failed to create tunnel: ${l}`);
    }
    let s = await t.json();
    if (this.connectionId = s.id, this.tunnelUrl = s.serverUrl, !this.connectionId || !this.tunnelUrl)
      throw new Error("Server did not return a valid connection ID or tunnel URL.");
    if (!s.tunnel?.token || !s.tunnel?.upstreamUrl)
      throw new Error("Tunnel configuration not available.");
    this.pikoClient = Bs({ endpointId: s.id, upstreamUrl: s.tunnel.upstreamUrl, token: s.tunnel.token, localAddr: this.localUrl.host, logger: { info: () => {}, warn: () => {}, error: () => {} } }), this.pikoClient.on("error", (a) => {
      let l = a instanceof Error ? a : new Error(String(a));
      this.emit("error", l);
    }), this.pikoClient.on("disconnected", () => {
      this._connected = false, this.emit("disconnected");
    });
    let n = new Promise((a, l) => {
      let c = setTimeout(() => l(new Error("Connection timeout")), 30000);
      this.pikoClient.on("connected", () => {
        clearTimeout(c), a();
      }), this.pikoClient.on("error", (h) => {
        clearTimeout(c), l(h);
      });
    });
    await this.pikoClient.start(), await n, this._connected = true, await this.activateTunnel();
    let i = this.options.syncIntervalMs ?? 5 * 60 * 1000;
    i > 0 && (this.syncTimer = setInterval(() => this.syncTools(), i));
    let o = this.info;
    if (!o)
      throw new Error("Tunnel connected but failed to retrieve connection info.");
    return this.emit("connected", o), o;
  }
  async stop() {
    if (this.syncTimer && (clearInterval(this.syncTimer), this.syncTimer = null), (this.options.cleanupOnStop ?? true) && this.connectionId)
      try {
        await this.fetchAuth({ path: `/mcp/connections/${this.connectionId}`, options: { method: "DELETE" } });
      } catch {}
    if (this.pikoClient) {
      try {
        await this.pikoClient.stop();
      } catch {}
      this.pikoClient = null;
    }
    this._connected = false, this.connectionId = null, this.tunnelUrl = null;
  }
  async createRecipe({ name: e } = {}) {
    if (!this.connectionId)
      throw new Error("Tunnel is not started.");
    let t = await this.fetchAuth({ path: `/mcp/connections/${this.connectionId}/create-recipe`, options: { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ name: e ?? this.options.name }) } });
    if (!t.ok)
      throw new Error(`Failed to create recipe (HTTP ${t.status}).`);
    return (await t.json()).link;
  }
  async activateTunnel() {
    let e = await this.fetchAuth({ path: `/mcp/connections/${this.connectionId}/activate-tunnel`, options: { method: "POST" } });
    if (e.ok) {
      let t = await e.json();
      t.status === "oauth_required" && t.authUrl && this.emit("oauthRequired", { authUrl: t.authUrl });
    } else
      await this.syncTools();
  }
  async syncTools() {
    try {
      let e = await this.fetchAuth({ path: `/mcp/connections/${this.connectionId}/sync-tools`, options: { method: "POST" } });
      if (e.ok) {
        let t = await e.json();
        if (t.requiresOAuth && t.oauthUrl)
          this.emit("oauthRequired", { authUrl: t.oauthUrl });
        else {
          let s = Array.isArray(t.tools) ? t.tools.length : 0;
          this.emit("toolsSynced", { toolCount: s });
        }
      }
    } catch {}
  }
};

// bridge/poke-bridge.ts
import * as readline from "node:readline";
import * as os from "node:os";
function emit(obj) {
  process.stdout.write(JSON.stringify(obj) + `
`);
}
function log(msg) {
  process.stderr.write(`\x1B[2m[bridge] ${msg}\x1B[0m
`);
}
async function ensureAuth() {
  if (!Wn()) {
    emit({ type: "auth_required", message: "Opening browser for Poke login…" });
    await Nn({ openBrowser: true });
  }
  const token = Bn();
  if (!token)
    throw new Error("Authentication failed: no token after login.");
  return token;
}
var argv = process.argv.slice(2);
function getArg(flag) {
  const i = argv.indexOf(flag);
  return i !== -1 && i + 1 < argv.length ? argv[i + 1] : null;
}
var mode = argv[0] ?? "tunnel";
async function runTunnel() {
  const mcpUrl = getArg("--mcp-url");
  if (!mcpUrl) {
    emit({ type: "error", message: "No --mcp-url provided to bridge." });
    process.exit(1);
  }
  const token = await ensureAuth();
  const poke = new et({ token });
  const tunnelName = `poke-around-${os.hostname().toLowerCase().replace(/[^a-z0-9]/g, "-")}`;
  let webhookUrl = null;
  let webhookToken = null;
  try {
    const wh = await poke.createWebhook({ condition: tunnelName, action: tunnelName });
    webhookUrl = wh.webhookUrl;
    webhookToken = wh.webhookToken;
    emit({ type: "webhook_ready", webhookUrl, webhookToken });
  } catch (err) {
    emit({ type: "webhook_error", message: String(err) });
  }
  const tunnel = new Gt({
    url: mcpUrl,
    name: tunnelName,
    token,
    cleanupOnStop: true
  });
  tunnel.on("connected", (info) => {
    emit({ type: "connected", connectionId: info.connectionId });
  });
  tunnel.on("disconnected", () => {
    emit({ type: "disconnected" });
  });
  tunnel.on("error", (err) => {
    emit({ type: "error", message: err.message });
  });
  tunnel.on("toolsSynced", ({ toolCount }) => {
    emit({ type: "tools_synced", count: toolCount });
  });
  await tunnel.start();
  log(`Tunnel started → ${mcpUrl}`);
  const rl = readline.createInterface({ input: process.stdin, terminal: false });
  rl.on("line", async (line) => {
    const trimmed = line.trim();
    if (!trimmed)
      return;
    try {
      const cmd = JSON.parse(trimmed);
      if (cmd.type === "send_webhook") {
        if (!webhookUrl || !webhookToken) {
          emit({ type: "webhook_error", message: "No webhook configured." });
          return;
        }
        try {
          await poke.sendWebhook({
            webhookUrl,
            webhookToken,
            data: { message: cmd.message }
          });
          emit({ type: "webhook_sent" });
        } catch (err) {
          emit({ type: "webhook_error", message: String(err) });
        }
      } else if (cmd.type === "stop") {
        log("Stop requested.");
        await tunnel.stop();
        process.exit(0);
      }
    } catch {}
  });
  rl.on("close", () => {
    tunnel.stop().finally(() => process.exit(0));
  });
}
async function runSendMessage() {
  const message = getArg("--message") ?? argv.slice(1).join(" ");
  if (!message) {
    process.stderr.write(`Usage: poke-bridge send-message --message "..."
`);
    process.exit(1);
  }
  const token = await ensureAuth();
  const poke = new et({ token });
  await poke.sendMessage(message);
  process.stdout.write(`sent
`);
}
if (mode === "send-message") {
  runSendMessage().catch((err) => {
    process.stderr.write(`bridge error: ${err.message}
`);
    process.exit(1);
  });
} else {
  runTunnel().catch((err) => {
    emit({ type: "error", message: err.message });
    process.exit(1);
  });
}
