"""Libraries supplied by the host OS/audio/graphics stack, rather than the image."""
HOST_LIBRARIES = frozenset({
    'libc.so.6','libm.so.6','libdl.so.2','libpthread.so.0','librt.so.1',
    'libresolv.so.2','libutil.so.1','libanl.so.1','ld-linux-x86-64.so.2',
    'libgcc_s.so.1','libstdc++.so.6','libGL.so.1','libGLX.so.0',
    'libGLdispatch.so.0','libEGL.so.1','libOpenGL.so.0','libGLESv2.so.2',
    'libglapi.so.0','libdrm.so.2','libgbm.so.1','libX11.so.6','libX11-xcb.so.1','libxcb.so.1',
    'libwayland-client.so.0','libwayland-server.so.0','libxkbcommon.so.0',
    'libasound.so.2','libpulse.so.0','libpulse-simple.so.0','libpulse-mainloop-glib.so.0',
    'libfontconfig.so.1','libffi.so.8','libz.so.1','libzstd.so.1',
})
