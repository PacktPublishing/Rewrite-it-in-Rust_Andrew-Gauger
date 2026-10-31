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
        __wbg___wbindgen_throw_344f42d3211c4765: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_length_0133fa10e3234c57: function(arg0) {
            const ret = getObject(arg0).length;
            return ret;
        },
        __wbg_new_from_slice_7e254b47c77fb8cc: function(arg0, arg1) {
            const ret = new Float64Array(getArrayF64FromWasm0(arg0, arg1));
            return addHeapObject(ret);
        },
        __wbg_prototypesetcall_21a175a0a8157491: function(arg0, arg1, arg2) {
            Float64Array.prototype.set.call(getArrayF64FromWasm0(arg0, arg1), getObject(arg2));
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

function getObject(idx) { return heap[idx]; }

let heap = new Array(1024).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

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

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedFloat64ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
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
