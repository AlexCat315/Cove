package com.cove.probe;

import android.os.Handler;
import android.os.Looper;
import java.lang.ref.WeakReference;

/** Diagnostics scene work remains responsive while the rendering surface is paused. */
final class Diagnostics {
    private static final Handler main = new Handler(Looper.getMainLooper());
    private static WeakReference<ContentSurfaceView> surface = new WeakReference<>(null);
    private static final Runnable poll = new Runnable() {
        @Override public void run() {
            ContentSurfaceView view = surface.get();
            if (view != null && view.pumpDiagnostics()) main.postDelayed(this, 100);
        }
    };
    static void attach(ContentSurfaceView view) {
        surface = new WeakReference<>(view);
        main.removeCallbacks(poll);
        main.post(poll);
    }
    static void detach(ContentSurfaceView view) {
        if (surface.get() == view) {
            main.removeCallbacks(poll);
            surface.clear();
        }
    }
}
