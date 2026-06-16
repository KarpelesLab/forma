//! Raw EGL (surfaceless) + OpenGL ES 2 offscreen blitter. Linux-only.
#![allow(unsafe_code)]
#![allow(non_snake_case, non_upper_case_globals)]

use core::ffi::{c_char, c_void};
use forma_geometry::{PhysicalSize, Rect};
use forma_render::{Color, Pixmap};

// ---- EGL / GL scalar types --------------------------------------------------

type EGLDisplay = *mut c_void;
type EGLConfig = *mut c_void;
type EGLContext = *mut c_void;
type EGLSurface = *mut c_void;
type EGLint = i32;
type EGLBoolean = u32;
type EGLenum = u32;
type EGLAttrib = isize;

type GLenum = u32;
type GLuint = u32;
type GLint = i32;
type GLsizei = i32;
type GLbitfield = u32;

const EGL_PLATFORM_SURFACELESS_MESA: EGLenum = 0x31DD;
const EGL_OPENGL_ES_API: EGLenum = 0x30A0;
const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;
const EGL_NONE: EGLint = 0x3038;
const EGL_SURFACE_TYPE: EGLint = 0x3033;
const EGL_PBUFFER_BIT: EGLint = 0x0001;
const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
const EGL_OPENGL_ES2_BIT: EGLint = 0x0004;
const EGL_RED_SIZE: EGLint = 0x3024;
const EGL_GREEN_SIZE: EGLint = 0x3023;
const EGL_BLUE_SIZE: EGLint = 0x3022;

const GL_TEXTURE_2D: GLenum = 0x0DE1;
const GL_TEXTURE0: GLenum = 0x84C0;
const GL_RGBA: GLenum = 0x1908;
const GL_UNSIGNED_BYTE: GLenum = 0x1401;
const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
const GL_TEXTURE_WRAP_T: GLenum = 0x2803;
const GL_LINEAR: GLint = 0x2601;
const GL_CLAMP_TO_EDGE: GLint = 0x812F;
const GL_FRAMEBUFFER: GLenum = 0x8D40;
const GL_COLOR_ATTACHMENT0: GLenum = 0x8CE0;
const GL_FRAMEBUFFER_COMPLETE: GLenum = 0x8CD5;
const GL_ARRAY_BUFFER: GLenum = 0x8892;
const GL_STATIC_DRAW: GLenum = 0x88E4;
const GL_FLOAT: GLenum = 0x1406;
const GL_FALSE: u8 = 0;
const GL_VERTEX_SHADER: GLenum = 0x8B31;
const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
const GL_COMPILE_STATUS: GLenum = 0x8B81;
const GL_LINK_STATUS: GLenum = 0x8B82;
const GL_TRIANGLE_STRIP: GLenum = 0x0005;
const GL_TRIANGLES: GLenum = 0x0004;
const GL_COLOR_BUFFER_BIT: GLbitfield = 0x4000;

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetPlatformDisplay(
        platform: EGLenum,
        native_display: *mut c_void,
        attrib_list: *const EGLAttrib,
    ) -> EGLDisplay;
    fn eglInitialize(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
    fn eglChooseConfig(
        dpy: EGLDisplay,
        attrib_list: *const EGLint,
        configs: *mut EGLConfig,
        config_size: EGLint,
        num_config: *mut EGLint,
    ) -> EGLBoolean;
    fn eglBindAPI(api: EGLenum) -> EGLBoolean;
    fn eglCreateContext(
        dpy: EGLDisplay,
        config: EGLConfig,
        share: EGLContext,
        attrib_list: *const EGLint,
    ) -> EGLContext;
    fn eglMakeCurrent(
        dpy: EGLDisplay,
        draw: EGLSurface,
        read: EGLSurface,
        ctx: EGLContext,
    ) -> EGLBoolean;
    fn eglGetError() -> EGLint;
}

#[link(name = "GLESv2")]
unsafe extern "C" {
    fn glGenTextures(n: GLsizei, textures: *mut GLuint);
    fn glBindTexture(target: GLenum, texture: GLuint);
    fn glTexParameteri(target: GLenum, pname: GLenum, param: GLint);
    fn glTexImage2D(
        target: GLenum,
        level: GLint,
        internalformat: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        format: GLenum,
        ty: GLenum,
        pixels: *const c_void,
    );
    fn glActiveTexture(texture: GLenum);
    fn glGenFramebuffers(n: GLsizei, framebuffers: *mut GLuint);
    fn glBindFramebuffer(target: GLenum, framebuffer: GLuint);
    fn glFramebufferTexture2D(
        target: GLenum,
        attachment: GLenum,
        textarget: GLenum,
        texture: GLuint,
        level: GLint,
    );
    fn glCheckFramebufferStatus(target: GLenum) -> GLenum;
    fn glViewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei);
    fn glClearColor(r: f32, g: f32, b: f32, a: f32);
    fn glClear(mask: GLbitfield);
    fn glCreateShader(ty: GLenum) -> GLuint;
    fn glShaderSource(
        shader: GLuint,
        count: GLsizei,
        string: *const *const c_char,
        length: *const GLint,
    );
    fn glCompileShader(shader: GLuint);
    fn glGetShaderiv(shader: GLuint, pname: GLenum, params: *mut GLint);
    fn glCreateProgram() -> GLuint;
    fn glAttachShader(program: GLuint, shader: GLuint);
    fn glLinkProgram(program: GLuint);
    fn glGetProgramiv(program: GLuint, pname: GLenum, params: *mut GLint);
    fn glUseProgram(program: GLuint);
    fn glGenBuffers(n: GLsizei, buffers: *mut GLuint);
    fn glBindBuffer(target: GLenum, buffer: GLuint);
    fn glBufferData(target: GLenum, size: isize, data: *const c_void, usage: GLenum);
    fn glGetAttribLocation(program: GLuint, name: *const c_char) -> GLint;
    fn glEnableVertexAttribArray(index: GLuint);
    fn glVertexAttribPointer(
        index: GLuint,
        size: GLint,
        ty: GLenum,
        normalized: u8,
        stride: GLsizei,
        pointer: *const c_void,
    );
    fn glGetUniformLocation(program: GLuint, name: *const c_char) -> GLint;
    fn glUniform1i(location: GLint, v0: GLint);
    fn glUniform1f(location: GLint, v0: f32);
    fn glUniform2f(location: GLint, v0: f32, v1: f32);
    fn glUniform4f(location: GLint, v0: f32, v1: f32, v2: f32, v3: f32);
    fn glDrawArrays(mode: GLenum, first: GLint, count: GLsizei);
    fn glReadPixels(
        x: GLint,
        y: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        ty: GLenum,
        pixels: *mut c_void,
    );
    fn glFinish();
}

const VERTEX_SRC: &[u8] = b"attribute vec2 pos;\nattribute vec2 uv;\nvarying vec2 v_uv;\nvoid main() {\n  v_uv = uv;\n  gl_Position = vec4(pos, 0.0, 1.0);\n}\n\0";
const FRAGMENT_SRC: &[u8] = b"precision mediump float;\nvarying vec2 v_uv;\nuniform sampler2D tex;\nvoid main() {\n  gl_FragColor = texture2D(tex, v_uv);\n}\n\0";

// Flat-color program for GPU-native geometry. The fragment shader evaluates a
// rounded-rectangle signed-distance field per pixel (a radius of 0 is a sharp
// rect), so a single program fills both sharp and rounded rectangles.
const FLAT_VERTEX_SRC: &[u8] =
    b"attribute vec2 pos;\nvoid main() {\n  gl_Position = vec4(pos, 0.0, 1.0);\n}\n\0";
const FLAT_FRAGMENT_SRC: &[u8] = b"precision mediump float;\n\
uniform vec4 u_color;\n\
uniform vec2 u_center;\n\
uniform vec2 u_half;\n\
uniform float u_radius;\n\
void main() {\n\
  vec2 p = gl_FragCoord.xy - u_center;\n\
  vec2 d = abs(p) - (u_half - vec2(u_radius));\n\
  float dist = length(max(d, vec2(0.0))) - u_radius;\n\
  if (dist > 0.5) discard;\n\
  gl_FragColor = u_color;\n\
}\n\0";

unsafe fn compile(kind: GLenum, src: &[u8]) -> Result<GLuint, String> {
    unsafe {
        let sh = glCreateShader(kind);
        let ptr = src.as_ptr() as *const c_char;
        glShaderSource(sh, 1, &ptr, core::ptr::null());
        glCompileShader(sh);
        let mut ok: GLint = 0;
        glGetShaderiv(sh, GL_COMPILE_STATUS, &mut ok);
        if ok == 0 {
            return Err(format!("shader compile failed (kind {kind:#x})"));
        }
        Ok(sh)
    }
}

/// Create a surfaceless EGL display + GLES2 context and make it current. Shared
/// by the present and GPU-native-draw paths; runs headlessly under Mesa.
unsafe fn egl_make_current() -> Result<(), String> {
    unsafe {
        let dpy = eglGetPlatformDisplay(
            EGL_PLATFORM_SURFACELESS_MESA,
            core::ptr::null_mut(),
            core::ptr::null(),
        );
        if dpy.is_null() {
            return Err("eglGetPlatformDisplay failed (no surfaceless EGL?)".into());
        }
        if eglInitialize(dpy, core::ptr::null_mut(), core::ptr::null_mut()) == 0 {
            return Err(format!("eglInitialize failed: {:#x}", eglGetError()));
        }
        eglBindAPI(EGL_OPENGL_ES_API);
        let cfg_attribs: [EGLint; 11] = [
            EGL_SURFACE_TYPE,
            EGL_PBUFFER_BIT,
            EGL_RENDERABLE_TYPE,
            EGL_OPENGL_ES2_BIT,
            EGL_RED_SIZE,
            8,
            EGL_GREEN_SIZE,
            8,
            EGL_BLUE_SIZE,
            8,
            EGL_NONE,
        ];
        let mut config: EGLConfig = core::ptr::null_mut();
        let mut num: EGLint = 0;
        if eglChooseConfig(dpy, cfg_attribs.as_ptr(), &mut config, 1, &mut num) == 0 || num == 0 {
            return Err(format!(
                "eglChooseConfig found no config: {:#x}",
                eglGetError()
            ));
        }
        let ctx_attribs: [EGLint; 3] = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE];
        let ctx = eglCreateContext(dpy, config, core::ptr::null_mut(), ctx_attribs.as_ptr());
        if ctx.is_null() {
            return Err(format!("eglCreateContext failed: {:#x}", eglGetError()));
        }
        if eglMakeCurrent(dpy, core::ptr::null_mut(), core::ptr::null_mut(), ctx) == 0 {
            return Err(format!(
                "eglMakeCurrent (surfaceless) failed: {:#x}",
                eglGetError()
            ));
        }
        Ok(())
    }
}

/// Read the current framebuffer back as a top-first [`Pixmap`] (GL is
/// bottom-up, so rows are flipped).
unsafe fn read_back(w: GLsizei, h: GLsizei) -> Pixmap {
    unsafe {
        let row = (w as usize) * 4;
        let mut raw = vec![0u8; row * h as usize];
        glReadPixels(
            0,
            0,
            w,
            h,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            raw.as_mut_ptr() as *mut c_void,
        );
        let mut out = vec![0u8; raw.len()];
        for y in 0..h as usize {
            let src = &raw[y * row..(y + 1) * row];
            let dst_y = h as usize - 1 - y;
            out[dst_y * row..(dst_y + 1) * row].copy_from_slice(src);
        }
        Pixmap::from_rgba8(PhysicalSize::new(w as u32, h as u32), out)
    }
}

/// Upload `input` to a texture, draw it to an offscreen framebuffer with a
/// pass-through shader, and read the result back as a new [`Pixmap`].
pub fn present_offscreen(input: &Pixmap) -> Result<Pixmap, String> {
    let size = input.size();
    let (w, h) = (size.width as GLsizei, size.height as GLsizei);
    if w == 0 || h == 0 {
        return Err("empty pixmap".into());
    }

    unsafe {
        egl_make_current()?;

        // --- input texture ---
        let mut src_tex: GLuint = 0;
        glGenTextures(1, &mut src_tex);
        glBindTexture(GL_TEXTURE_2D, src_tex);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            GL_RGBA as GLint,
            w,
            h,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            input.as_bytes().as_ptr() as *const c_void,
        );

        // --- offscreen color target + FBO ---
        let mut dst_tex: GLuint = 0;
        glGenTextures(1, &mut dst_tex);
        glBindTexture(GL_TEXTURE_2D, dst_tex);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            GL_RGBA as GLint,
            w,
            h,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            core::ptr::null(),
        );
        let mut fbo: GLuint = 0;
        glGenFramebuffers(1, &mut fbo);
        glBindFramebuffer(GL_FRAMEBUFFER, fbo);
        glFramebufferTexture2D(
            GL_FRAMEBUFFER,
            GL_COLOR_ATTACHMENT0,
            GL_TEXTURE_2D,
            dst_tex,
            0,
        );
        if glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE {
            return Err("framebuffer incomplete".into());
        }
        glViewport(0, 0, w, h);

        // --- program ---
        let vs = compile(GL_VERTEX_SHADER, VERTEX_SRC)?;
        let fs = compile(GL_FRAGMENT_SHADER, FRAGMENT_SRC)?;
        let prog = glCreateProgram();
        glAttachShader(prog, vs);
        glAttachShader(prog, fs);
        glLinkProgram(prog);
        let mut linked: GLint = 0;
        glGetProgramiv(prog, GL_LINK_STATUS, &mut linked);
        if linked == 0 {
            return Err("program link failed".into());
        }
        glUseProgram(prog);

        // --- fullscreen quad (pos.xy, uv.xy); uv row 0 = texture top ---
        #[rustfmt::skip]
        let verts: [f32; 16] = [
            -1.0, -1.0, 0.0, 1.0,
             1.0, -1.0, 1.0, 1.0,
            -1.0,  1.0, 0.0, 0.0,
             1.0,  1.0, 1.0, 0.0,
        ];
        let mut vbo: GLuint = 0;
        glGenBuffers(1, &mut vbo);
        glBindBuffer(GL_ARRAY_BUFFER, vbo);
        glBufferData(
            GL_ARRAY_BUFFER,
            std::mem::size_of_val(&verts) as isize,
            verts.as_ptr() as *const c_void,
            GL_STATIC_DRAW,
        );
        let stride = 4 * std::mem::size_of::<f32>() as GLsizei;
        let pos_loc = glGetAttribLocation(prog, c"pos".as_ptr());
        let uv_loc = glGetAttribLocation(prog, c"uv".as_ptr());
        if pos_loc < 0 || uv_loc < 0 {
            return Err("attribute location not found".into());
        }
        glEnableVertexAttribArray(pos_loc as GLuint);
        glVertexAttribPointer(
            pos_loc as GLuint,
            2,
            GL_FLOAT,
            GL_FALSE,
            stride,
            core::ptr::null(),
        );
        glEnableVertexAttribArray(uv_loc as GLuint);
        glVertexAttribPointer(
            uv_loc as GLuint,
            2,
            GL_FLOAT,
            GL_FALSE,
            stride,
            (2 * std::mem::size_of::<f32>()) as *const c_void,
        );

        // --- draw the textured quad ---
        glActiveTexture(GL_TEXTURE0);
        glBindTexture(GL_TEXTURE_2D, src_tex);
        let tex_loc = glGetUniformLocation(prog, c"tex".as_ptr());
        glUniform1i(tex_loc, 0);
        glClearColor(0.0, 0.0, 0.0, 1.0);
        glClear(GL_COLOR_BUFFER_BIT);
        glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
        glFinish();
        Ok(read_back(w, h))
    }
}

/// Draw solid-color rectangles **GPU-natively** — as tessellated geometry
/// through a flat-color shader (not by compositing a CPU pixmap) — onto a
/// `background`-cleared offscreen framebuffer, and read the result back.
///
/// Each entry is `(rect, color, corner_radius)` in logical pixels with a
/// top-left origin (mapped to GL NDC); the fragment shader evaluates a
/// rounded-rectangle SDF, so a radius of `0.0` is a sharp rect. This is the
/// first step of the GPU-native scene renderer: borders and a glyph atlas are
/// future work.
pub fn fill_rects_offscreen(
    size: PhysicalSize,
    background: Color,
    rects: &[(Rect, Color, f32)],
) -> Result<Pixmap, String> {
    let (w, h) = (size.width as GLsizei, size.height as GLsizei);
    if w == 0 || h == 0 {
        return Err("empty target".into());
    }
    let (wf, hf) = (size.width as f32, size.height as f32);

    unsafe {
        egl_make_current()?;

        // --- offscreen color target + FBO ---
        let mut dst_tex: GLuint = 0;
        glGenTextures(1, &mut dst_tex);
        glBindTexture(GL_TEXTURE_2D, dst_tex);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            GL_RGBA as GLint,
            w,
            h,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            core::ptr::null(),
        );
        let mut fbo: GLuint = 0;
        glGenFramebuffers(1, &mut fbo);
        glBindFramebuffer(GL_FRAMEBUFFER, fbo);
        glFramebufferTexture2D(
            GL_FRAMEBUFFER,
            GL_COLOR_ATTACHMENT0,
            GL_TEXTURE_2D,
            dst_tex,
            0,
        );
        if glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE {
            return Err("framebuffer incomplete".into());
        }
        glViewport(0, 0, w, h);

        // --- flat-color program ---
        let vs = compile(GL_VERTEX_SHADER, FLAT_VERTEX_SRC)?;
        let fs = compile(GL_FRAGMENT_SHADER, FLAT_FRAGMENT_SRC)?;
        let prog = glCreateProgram();
        glAttachShader(prog, vs);
        glAttachShader(prog, fs);
        glLinkProgram(prog);
        let mut linked: GLint = 0;
        glGetProgramiv(prog, GL_LINK_STATUS, &mut linked);
        if linked == 0 {
            return Err("flat program link failed".into());
        }
        glUseProgram(prog);
        let pos_loc = glGetAttribLocation(prog, c"pos".as_ptr());
        let color_loc = glGetUniformLocation(prog, c"u_color".as_ptr());
        let center_loc = glGetUniformLocation(prog, c"u_center".as_ptr());
        let half_loc = glGetUniformLocation(prog, c"u_half".as_ptr());
        let radius_loc = glGetUniformLocation(prog, c"u_radius".as_ptr());
        if pos_loc < 0 {
            return Err("flat attribute location not found".into());
        }

        let mut vbo: GLuint = 0;
        glGenBuffers(1, &mut vbo);
        glBindBuffer(GL_ARRAY_BUFFER, vbo);
        glEnableVertexAttribArray(pos_loc as GLuint);
        glVertexAttribPointer(
            pos_loc as GLuint,
            2,
            GL_FLOAT,
            GL_FALSE,
            0,
            core::ptr::null(),
        );

        // Clear to the background, then draw each rect as two triangles.
        let bg = [
            background.r as f32 / 255.0,
            background.g as f32 / 255.0,
            background.b as f32 / 255.0,
            background.a as f32 / 255.0,
        ];
        glClearColor(bg[0], bg[1], bg[2], bg[3]);
        glClear(GL_COLOR_BUFFER_BIT);

        // logical (top-left origin) → GL NDC.
        let ndc = |x: f64, y: f64| (2.0 * x as f32 / wf - 1.0, 1.0 - 2.0 * y as f32 / hf);
        for (rect, color, radius) in rects {
            let (x0, y0) = ndc(rect.min_x(), rect.min_y());
            let (x1, y1) = ndc(rect.max_x(), rect.max_y());
            // SDF uniforms in framebuffer pixels (origin bottom-left, so flip y).
            let (rw, rh) = (rect.width() as f32, rect.height() as f32);
            let cx = rect.min_x() as f32 + rw / 2.0;
            let cy = hf - (rect.min_y() as f32 + rh / 2.0);
            let r = radius.clamp(0.0, rw.min(rh) / 2.0);
            glUniform2f(center_loc, cx, cy);
            glUniform2f(half_loc, rw / 2.0, rh / 2.0);
            glUniform1f(radius_loc, r);
            #[rustfmt::skip]
            let verts: [f32; 12] = [
                x0, y0,  x1, y0,  x0, y1,
                x0, y1,  x1, y0,  x1, y1,
            ];
            glBufferData(
                GL_ARRAY_BUFFER,
                std::mem::size_of_val(&verts) as isize,
                verts.as_ptr() as *const c_void,
                GL_STATIC_DRAW,
            );
            glVertexAttribPointer(
                pos_loc as GLuint,
                2,
                GL_FLOAT,
                GL_FALSE,
                0,
                core::ptr::null(),
            );
            glUniform4f(
                color_loc,
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                color.a as f32 / 255.0,
            );
            glDrawArrays(GL_TRIANGLES, 0, 6);
        }
        glFinish();
        Ok(read_back(w, h))
    }
}
