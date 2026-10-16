package xyz.joeyshi.lf2showdown;

import android.app.NativeActivity;
import android.content.Intent;
import android.os.Build;
import android.os.Process;
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
 */
public class MainActivity extends NativeActivity {
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
