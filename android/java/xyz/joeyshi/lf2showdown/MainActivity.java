package xyz.joeyshi.lf2showdown;

import android.app.NativeActivity;
import android.content.Intent;
import android.hardware.input.InputManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Process;
import android.view.InputDevice;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;

/**
 * The game's NativeActivity, with the status bar and the navigation bar hidden while
 * the game has the focus. A swipe from the edge of the screen shows them for a while.
 *
 * The game runs once per process: winit makes one event loop per process, and the
 * game does not stop when the activity is destroyed. So onDestroy ends the process.
 * When Android is recreating the activity (after a change it always recreates
 * activities for, such as the font size, the wallpaper or an update of the app), it
 * first starts RestartActivity in another process, which starts the game again.
 *
 * It also tells the game whether a game controller is connected, when it starts and
 * whenever a device comes or goes, so that the game can hide its touch controls.
 */
public class MainActivity extends NativeActivity implements InputManager.InputDeviceListener {
    static {
        // NativeActivity loads the library itself; loading it here as well lets Java
        // find nativeControllerConnected in it.
        System.loadLibrary("lf2_android");
    }

    /** crates/lf2-android/src/input.rs */
    private static native void nativeControllerConnected(boolean connected);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        getSystemService(InputManager.class).registerInputDeviceListener(this, null);
        reportControllers();
    }

    @Override
    public void onInputDeviceAdded(int deviceId) {
        reportControllers();
    }

    @Override
    public void onInputDeviceRemoved(int deviceId) {
        reportControllers();
    }

    @Override
    public void onInputDeviceChanged(int deviceId) {
    }

    /** What reportControllers last reported; null before the first report. */
    private Boolean reported;

    /** Report whether a controller is connected, when that changes. */
    private void reportControllers() {
        boolean connected = false;
        for (int id : InputDevice.getDeviceIds()) {
            InputDevice d = InputDevice.getDevice(id);
            if (d == null || d.isVirtual()) {
                continue;
            }
            int s = d.getSources();
            if ((s & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD
                    || (s & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK) {
                connected = true;
            }
        }
        if (reported == null || reported != connected) {
            reported = connected;
            nativeControllerConnected(connected);
        }
    }

    @Override
    protected void onDestroy() {
        if (isChangingConfigurations()) {
            startActivity(new Intent(this, RestartActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        }
        Process.killProcess(Process.myPid());
        super.onDestroy();
    }
    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            hideSystemBars();
        }
    }

    @SuppressWarnings("deprecation")
    private void hideSystemBars() {
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController c = getWindow().getInsetsController();
            if (c != null) {
                c.hide(WindowInsets.Type.systemBars());
                c.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        } else {
            getWindow().getDecorView().setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    | View.SYSTEM_UI_FLAG_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                    | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN);
        }
    }
}
