/**
 * The full sensor point cloud, copied into WASM linear memory exactly once
 * at page load. Every later query (time slice, sensor selection, viewport)
 * runs as filters over this resident copy instead of re-crossing the
 * JS/WASM boundary with fresh data.
 */
export class PointCloud {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        PointCloudFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_pointcloud_free(ptr, 0);
    }
    /**
     * A kernel-density heatmap grid over the viewport at instant `at_ts`:
     * each active sensor's interpolated reading is projected into pixel
     * space (Web Mercator, at the given `zoom`) and contributes a smooth
     * Gaussian kernel to its neighboring cells, sized by its own
     * `vibration_g` magnitude, blended with `temp_f` into one intensity
     * value. This turns the filtered point set into density numbers on a
     * projected plane -- it does not decide colors or paint any pixels;
     * that's a later stage.
     *
     * The returned `Float64Array` is `[cols, rows, cell(0,0), cell(0,1),
     * ..., cell(rows-1, cols-1)]`: a 2-value header giving the grid's
     * dimensions (so the caller doesn't have to re-derive them from the
     * viewport/zoom/cell size itself), followed by `cols * rows` row-major
     * density values. Grid cells are `cell_size_px` pixels square.
     *
     * Known limitation: a reading just outside the requested viewport is
     * dropped before projection, even though a high-vibration reading's
     * kernel could otherwise bleed a little heat into the grid's visible
     * edge cells. This matches `range`/`snapshot_at`'s existing hard cutoff
     * at the viewport boundary today, so it's not a regression -- just
     * known edge softness, fixable later by padding the *selection* bbox
     * without changing this grid's dimensions.
     * @param {number} at_ts
     * @param {number} sensor_mask
     * @param {number} min_lat
     * @param {number} min_lng
     * @param {number} max_lat
     * @param {number} max_lng
     * @param {number} zoom
     * @param {number} cell_size_px
     * @returns {Float64Array}
     */
    density_grid(at_ts, sensor_mask, min_lat, min_lng, max_lat, max_lng, zoom, cell_size_px) {
        const ret = wasm.pointcloud_density_grid(this.__wbg_ptr, at_ts, sensor_mask, min_lat, min_lng, max_lat, max_lng, zoom, cell_size_px);
        return takeObject(ret);
    }
    /**
     * @returns {number}
     */
    len() {
        const ret = wasm.pointcloud_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * `flat` is a row-major Float64Array: one bulk copy out of JS memory
     * (`Float64Array::to_vec`), not a per-property call per point.
     * @param {Float64Array} flat
     * @param {number} sensor_count
     */
    constructor(flat, sensor_count) {
        try {
            const ret = wasm.pointcloud_new(addBorrowedObject(flat), sensor_count);
            this.__wbg_ptr = ret;
            PointCloudFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            heap[stack_pointer++] = undefined;
        }
    }
    /**
     * Paints the same kernel-density heatmap `density_grid` computes
     * directly onto `canvas`, from Rust, in one call: project, KDE,
     * palette, paint end to end. No JavaScript touches a pixel -- the
     * pixel buffer is built as a `Clamped<&[u8]>` (mapping straight onto
     * a `Uint8ClampedArray`, no separate JS glue), wrapped in an
     * `ImageData`, and blitted with `put_image_data`.
     *
     * The canvas's own drawing buffer is resized (`set_width`/
     * `set_height`) to the density grid's `cols`/`rows` -- one device
     * pixel per KDE cell, not one per screen pixel. `cell_size_px` is
     * already the resolution the Gaussian smoothing is visually
     * meaningful at; painting at full viewport resolution would rerun
     * KDE far more densely for a blur that's already smoother than that
     * per cell. Stretch the small buffer to the viewport's actual size
     * with the canvas element's CSS `width`/`height` and let the
     * browser's own bilinear upscale do the rest.
     *
     * Pan, zoom, and scrubbing the time slider each call this same
     * entry point with whatever changed (bbox, zoom, or `at_ts`/
     * `sensor_mask`) -- one pipeline, three triggers. Deliberately not
     * solved here: a pure pan with no time/sensor change recomputes
     * density that hasn't actually changed. Caching the density grid
     * and only reprojecting/repainting against it is a known, flagged
     * optimization; this ships the correct version first.
     * @param {HTMLCanvasElement} canvas
     * @param {number} at_ts
     * @param {number} sensor_mask
     * @param {number} min_lat
     * @param {number} min_lng
     * @param {number} max_lat
     * @param {number} max_lng
     * @param {number} zoom
     * @param {number} cell_size_px
     */
    paint_heatmap(canvas, at_ts, sensor_mask, min_lat, min_lng, max_lat, max_lng, zoom, cell_size_px) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.pointcloud_paint_heatmap(retptr, this.__wbg_ptr, addBorrowedObject(canvas), at_ts, sensor_mask, min_lat, min_lng, max_lat, max_lng, zoom, cell_size_px);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            heap[stack_pointer++] = undefined;
        }
    }
    /**
     * Draws each active sensor's raw readings as text at its projected
     * position -- the numbers actually driving `paint_heatmap`'s color,
     * not the color itself. A separate canvas from `paint_heatmap`'s on
     * purpose: that canvas is deliberately sized to the KDE grid's own
     * (coarse) resolution and CSS-stretched, which would make text blurry;
     * this one is sized to the bbox's full projected pixel resolution so
     * labels stay crisp.
     * @param {HTMLCanvasElement} canvas
     * @param {number} at_ts
     * @param {number} sensor_mask
     * @param {number} min_lat
     * @param {number} min_lng
     * @param {number} max_lat
     * @param {number} max_lng
     * @param {number} zoom
     * @param {string[]} sensor_names
     */
    paint_heatmap_labels(canvas, at_ts, sensor_mask, min_lat, min_lng, max_lat, max_lng, zoom, sensor_names) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayJsValueToWasm0(sensor_names, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.pointcloud_paint_heatmap_labels(retptr, this.__wbg_ptr, addBorrowedObject(canvas), at_ts, sensor_mask, min_lat, min_lng, max_lat, max_lng, zoom, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            heap[stack_pointer++] = undefined;
        }
    }
    /**
     * Every raw reading matching a sensor bitmask, a timestamp range, and a
     * viewport bounding box. Used by "show all readings" mode, scoped down
     * to whatever the customer has actually selected and panned to.
     * @param {number} sensor_mask
     * @param {number} start_ts
     * @param {number} end_ts
     * @param {number} min_lat
     * @param {number} min_lng
     * @param {number} max_lat
     * @param {number} max_lng
     * @returns {Float64Array}
     */
    range(sensor_mask, start_ts, end_ts, min_lat, min_lng, max_lat, max_lng) {
        const ret = wasm.pointcloud_range(this.__wbg_ptr, sensor_mask, start_ts, end_ts, min_lat, min_lng, max_lat, max_lng);
        return takeObject(ret);
    }
    /**
     * One interpolated row per active sensor at instant `at_ts`, linearly
     * blended between the two readings bracketing it. This is the time
     * slider's "video playback" path: scrubbing asks for a single instant,
     * not the raw once-a-minute cadence, so motion reads as continuous.
     *
     * Each returned row is `[sensor_idx, lat, lng, temp_f, vibration_g, at_ts]`
     * (`ROW_LEN` = 6 floats), flattened into one `Float64Array`.
     *
     * # Examples
     *
     * Called from the frontend on every slider tick, with the map's current
     * viewport and sensor selection as the filter:
     *
     * ```js
     * const bounds = map.getBounds();
     * const rows = cloud.snapshot_at(
     *   atTs, sensorMask,
     *   bounds.getSouth(), bounds.getWest(), bounds.getNorth(), bounds.getEast(),
     * );
     * for (let i = 0; i < rows.length; i += 6) {
     *   const [sensorIdx, lat, lng, tempF, vibrationG] = rows.slice(i, i + 6);
     *   // ...place a marker at [lat, lng]
     * }
     * ```
     * @param {number} at_ts
     * @param {number} sensor_mask
     * @param {number} min_lat
     * @param {number} min_lng
     * @param {number} max_lat
     * @param {number} max_lng
     * @returns {Float64Array}
     */
    snapshot_at(at_ts, sensor_mask, min_lat, min_lng, max_lat, max_lng) {
        const ret = wasm.pointcloud_snapshot_at(this.__wbg_ptr, at_ts, sensor_mask, min_lat, min_lng, max_lat, max_lng);
        return takeObject(ret);
    }
}
if (Symbol.dispose) PointCloud.prototype[Symbol.dispose] = PointCloud.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_string_get_b0ca35b86a603356: function(arg0, arg1) {
            const obj = getObject(arg1);
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_344f42d3211c4765: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_fillText_e462ba58cec15054: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).fillText(getStringFromWasm0(arg1, arg2), arg3, arg4);
        }, arguments); },
        __wbg_getContext_e79ddf6a9cb3cc76: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = getObject(arg0).getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        }, arguments); },
        __wbg_instanceof_CanvasRenderingContext2d_2284b703b7023dcc: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof CanvasRenderingContext2D;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_length_0133fa10e3234c57: function(arg0) {
            const ret = getObject(arg0).length;
            return ret;
        },
        __wbg_new_from_slice_7e254b47c77fb8cc: function(arg0, arg1) {
            const ret = new Float64Array(getArrayF64FromWasm0(arg0, arg1));
            return addHeapObject(ret);
        },
        __wbg_new_with_u8_clamped_array_and_sh_2767e4741c267d25: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = new ImageData(getClampedArrayU8FromWasm0(arg0, arg1), arg2 >>> 0, arg3 >>> 0);
            return addHeapObject(ret);
        }, arguments); },
        __wbg_prototypesetcall_21a175a0a8157491: function(arg0, arg1, arg2) {
            Float64Array.prototype.set.call(getArrayF64FromWasm0(arg0, arg1), getObject(arg2));
        },
        __wbg_putImageData_a4dee11e08ab9ac8: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            getObject(arg0).putImageData(getObject(arg1), arg2, arg3);
        }, arguments); },
        __wbg_set_fillStyle_4360b989b9352bbb: function(arg0, arg1, arg2) {
            getObject(arg0).fillStyle = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_font_33fee74f2c82cb6f: function(arg0, arg1, arg2) {
            getObject(arg0).font = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_height_7d9d8f892e6964c6: function(arg0, arg1) {
            getObject(arg0).height = arg1 >>> 0;
        },
        __wbg_set_lineWidth_beb3d05e36f4cc53: function(arg0, arg1) {
            getObject(arg0).lineWidth = arg1;
        },
        __wbg_set_strokeStyle_b390d5f09a6989a8: function(arg0, arg1, arg2) {
            getObject(arg0).strokeStyle = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_textAlign_75f93b22c0415d5d: function(arg0, arg1, arg2) {
            getObject(arg0).textAlign = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_width_8e30d010cd66830d: function(arg0, arg1) {
            getObject(arg0).width = arg1 >>> 0;
        },
        __wbg_strokeText_a6f6e40f45e3f29b: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).strokeText(getStringFromWasm0(arg1, arg2), arg3, arg4);
        }, arguments); },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return addHeapObject(ret);
        },
        __wbindgen_object_drop_ref: function(arg0) {
            takeObject(arg0);
        },
    };
    return {
        __proto__: null,
        "./viz_core_bg.js": import0,
    };
}

const PointCloudFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_pointcloud_free(ptr, 1));

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function addBorrowedObject(obj) {
    if (stack_pointer == 1) throw new Error('out of js stack');
    heap[--stack_pointer] = obj;
    return stack_pointer;
}

function dropObject(idx) {
    if (idx < 1028) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function getArrayF64FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}

function getClampedArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ClampedArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
    if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
        cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

let cachedUint8ClampedArrayMemory0 = null;
function getUint8ClampedArrayMemory0() {
    if (cachedUint8ClampedArrayMemory0 === null || cachedUint8ClampedArrayMemory0.byteLength === 0) {
        cachedUint8ClampedArrayMemory0 = new Uint8ClampedArray(wasm.memory.buffer);
    }
    return cachedUint8ClampedArrayMemory0;
}

function getObject(idx) { return heap[idx]; }

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        wasm.__wbindgen_export3(addHeapObject(e));
    }
}

let heap = new Array(1024).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    const mem = getDataViewMemory0();
    for (let i = 0; i < array.length; i++) {
        mem.setUint32(ptr + 4 * i, addHeapObject(array[i]), true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

let stack_pointer = 1024;

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat64ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    cachedUint8ClampedArrayMemory0 = null;
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('viz_core_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
