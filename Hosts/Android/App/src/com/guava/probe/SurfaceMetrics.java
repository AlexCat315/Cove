package com.guava.probe;

import android.content.res.Configuration;
import android.graphics.Rect;
import android.os.Build;
import android.util.DisplayMetrics;
import android.util.TypedValue;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import java.util.List;
import java.util.function.LongSupplier;

/** Owns native surface facts, including updates while the IME animates. */
final class SurfaceMetrics {
    private final View view;
    private final LongSupplier session;
    private WindowInsets insets;

    SurfaceMetrics(View view, LongSupplier session) {
        this.view = view;
        this.session = session;
        if (Build.VERSION.SDK_INT >= 30) {
            view.setWindowInsetsAnimationCallback(new WindowInsetsAnimation.Callback(
                    WindowInsetsAnimation.Callback.DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
                @Override public WindowInsets onProgress(WindowInsets value, List<WindowInsetsAnimation> running) {
                    update(value);
                    return value;
                }
                @Override public void onEnd(WindowInsetsAnimation animation) {
                    // onProgress need not deliver the final target after an
                    // interrupted animation. Read canonical insets next turn.
                    view.post(() -> refresh());
                }
            });
        }
    }

    void update(WindowInsets value) { insets = value; publish(); }

    void refresh() {
        WindowInsets current = view.getRootWindowInsets();
        if (current != null) insets = current;
        publish();
    }

    private void publish() {
        long target = session.getAsLong();
        if (target == 0) return;
        Rect safe = new Rect();
        int keyboard = 0;
        if (insets != null) {
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets bars = insets.getInsets(
                        WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                safe.set(bars.left, bars.top, bars.right, bars.bottom);
                keyboard = insets.getInsets(WindowInsets.Type.ime()).bottom;
            } else {
                // Legacy system insets combine bars and IME. Stable insets
                // retain the bar floor while the keyboard is visible.
                int bottom = insets.getSystemWindowInsetBottom();
                int barBottom = insets.getStableInsetBottom();
                keyboard = bottom > barBottom ? bottom : 0;
                safe.set(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                         insets.getSystemWindowInsetRight(), Math.min(bottom, barBottom));
                android.view.DisplayCutout cutout = insets.getDisplayCutout();
                if (cutout != null) {
                    safe.left = Math.max(safe.left, cutout.getSafeInsetLeft());
                    safe.top = Math.max(safe.top, cutout.getSafeInsetTop());
                    safe.right = Math.max(safe.right, cutout.getSafeInsetRight());
                    safe.bottom = Math.max(safe.bottom, cutout.getSafeInsetBottom());
                }
            }
        }
        Configuration configuration = view.getResources().getConfiguration();
        DisplayMetrics display = view.getResources().getDisplayMetrics();
        float density = Float.isFinite(display.density) && display.density > 0 ? display.density : 1;
        boolean rtl = configuration.getLayoutDirection() == View.LAYOUT_DIRECTION_RTL;
        MainActivity.surface(target, density, configuration.smallestScreenWidthDp >= 600 ? 1 : 0,
                safe.top / density, (rtl ? safe.right : safe.left) / density,
                safe.bottom / density, (rtl ? safe.left : safe.right) / density,
                keyboard / density, textSizes(display, density));
    }

    private static float[] textSizes(DisplayMetrics display, float density) {
        // Native SP conversion preserves Android 14's nonlinear accessibility
        // ramp. Pairs cross JNI as (authored points, scaled logical points).
        float[] sizes = {8, 10, 12, 14, 16, 18, 20, 24, 28, 32, 40, 48, 64, 96};
        float[] samples = new float[sizes.length * 2];
        for (int i = 0; i < sizes.length; i++) {
            samples[i * 2] = sizes[i];
            samples[i * 2 + 1] = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, sizes[i], display) / density;
        }
        return samples;
    }
}
