package afeather.kaulan

import android.content.ContentResolver
import android.content.Intent
import android.database.Cursor
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.provider.OpenableColumns
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import org.json.JSONObject
import java.util.concurrent.CompletableFuture
import java.util.concurrent.TimeUnit

class MainActivity : TauriActivity() {
  private companion object {
    private const val HIDDEN_SOLVER_USER_AGENT =
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
        "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    private const val HIDDEN_SOLVER_HTML = """
      <!DOCTYPE html>
      <html lang="en">
      <head>
        <meta charset="utf-8" />
        <script src="ytdl/meriyah.umd.min.js"></script>
        <script src="ytdl/astring.min.js"></script>
      </head>
      <body></body>
      </html>
    """
  }

  private external fun nativeInitAndroidContext()
  private external fun nativeReleaseAndroidContext()
  // Forward the OS-launched audio file (content:// or file:// URI) to the
  // backend launch broker. `displayName` is the friendly filename from
  // ContentResolver (null when not resolvable). See docs/default-music-app.md.
  private external fun nativeSetLaunchFile(uri: String, displayName: String?)

  private val mainHandler = Handler(Looper.getMainLooper())
  private val hiddenSolverLock = Object()
  private var hiddenSolverWebView: WebView? = null
  private var hiddenSolverReady = false
  private var hiddenSolverLoading = false

  fun runHiddenSolver(inputJson: String, coreCode: String): String? {
    if (!awaitHiddenSolverReady(30_000L)) {
      return null
    }
    val initStatus = evaluateHiddenSolver(
      """
        (() => {
          if (window.__ytdlSolveReady) {
            return { ready: true, error: null };
          }
          if (!window.meriyah || !window.astring) {
            return { ready: false, error: "solver dependencies not loaded" };
          }
          try {
            const source = ${JSONObject.quote(coreCode)};
            window.__ytdlSolve = eval(`${'$'}{source}\n; jsc;`);
            window.__ytdlSolveReady = true;
            window.__ytdlSolveError = null;
            return { ready: true, error: null };
          } catch (error) {
            const message = String(error && error.stack ? error.stack : error);
            window.__ytdlSolveError = message;
            return { ready: false, error: message };
          }
        })()
      """.trimIndent(),
      30_000L,
    ) ?: return null
    val statusJson = JSONObject(initStatus)
    if (!statusJson.optBoolean("ready")) {
      return null
    }
    return evaluateHiddenSolver(
      """
        (() => {
          try {
            const input = JSON.parse(${JSONObject.quote(inputJson)});
            return window.__ytdlSolve(input);
          } catch (error) {
            return {
              type: "error",
              error: String(error && error.stack ? error.stack : error),
            };
          }
        })()
      """.trimIndent(),
      30_000L,
    )
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    nativeInitAndroidContext()
    // Cold-start case: capture the VIEW intent the OS launched us with. The
    // backend broker is a static OnceLock in the Rust crate, so this is safe
    // to call before the in-process backend binds its port.
    handleLaunchIntent(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    // Warm-start case: Kaulan is already running (singleTask launch mode).
    // Forward the new VIEW intent's URI to the broker; the frontend's SSE
    // subscription picks it up.
    handleLaunchIntent(intent)
  }

  private fun handleLaunchIntent(intent: Intent?) {
    if (intent == null || Intent.ACTION_VIEW != intent.action) {
      return
    }
    val data: Uri = intent.data ?: return
    val uriString = data.toString()
    val displayName = resolveDisplayName(data)
    try {
      nativeSetLaunchFile(uriString, displayName)
      android.util.Log.i(
        "KaulanLaunch",
        "Forwarded launch URI: $uriString (name=$displayName)",
      )
    } catch (e: Throwable) {
      android.util.Log.e("KaulanLaunch", "nativeSetLaunchFile failed", e)
    }
  }

  /**
   * Resolve a friendly filename for the launch URI via ContentResolver's
   * OpenableColumns. Returns null when the column isn't available (the
   * frontend then falls back to the URI's last path segment).
   */
  private fun resolveDisplayName(uri: Uri): String? {
    if (uri.scheme != ContentResolver.SCHEME_CONTENT) {
      // For file:// URIs the path itself ends in a filename; let the
      // frontend derive it.
      return null
    }
    val resolver = contentResolver ?: return null
    return try {
      resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { cursor: Cursor ->
          if (cursor.moveToFirst() && !cursor.isNull(0)) {
            cursor.getString(0)
          } else {
            null
          }
        }
    } catch (e: Throwable) {
      android.util.Log.w("KaulanLaunch", "DISPLAY_NAME query failed", e)
      null
    }
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    webView.settings.userAgentString = HIDDEN_SOLVER_USER_AGENT
    webView.settings.setSupportZoom(true)
    webView.settings.builtInZoomControls = true
    webView.settings.displayZoomControls = false
    webView.settings.useWideViewPort = true
    webView.settings.loadWithOverviewMode = true
  }

  override fun onDestroy() {
    destroyHiddenSolverWebView()
    nativeReleaseAndroidContext()
    super.onDestroy()
  }

  private fun awaitHiddenSolverReady(timeoutMs: Long): Boolean {
    if (Looper.myLooper() == Looper.getMainLooper()) {
      ensureHiddenSolverLoadedOnMainThread()
      synchronized(hiddenSolverLock) {
        return hiddenSolverReady && hiddenSolverWebView != null
      }
    }
    val deadline = SystemClock.uptimeMillis() + timeoutMs
    synchronized(hiddenSolverLock) {
      mainHandler.post { ensureHiddenSolverLoadedOnMainThread() }
      while (!hiddenSolverReady || hiddenSolverWebView == null) {
        val remainingMs = deadline - SystemClock.uptimeMillis()
        if (remainingMs <= 0L) {
          return false
        }
        try {
          hiddenSolverLock.wait(remainingMs)
        } catch (_: InterruptedException) {
          Thread.currentThread().interrupt()
          return false
        }
      }
      return true
    }
  }

  private fun ensureHiddenSolverLoadedOnMainThread() {
    check(Looper.myLooper() == Looper.getMainLooper())
    val solverWebView: WebView
    synchronized(hiddenSolverLock) {
      if (hiddenSolverWebView == null) {
        hiddenSolverWebView = WebView(this)
        configureHiddenSolverWebView(hiddenSolverWebView!!)
      }
      if (hiddenSolverReady || hiddenSolverLoading) {
        return
      }
      solverWebView = hiddenSolverWebView!!
      hiddenSolverReady = false
      hiddenSolverLoading = true
    }
    solverWebView.loadDataWithBaseURL(
      "file:///android_asset/",
      HIDDEN_SOLVER_HTML,
      "text/html",
      "utf-8",
      null,
    )
  }

  private fun configureHiddenSolverWebView(webView: WebView) {
    val settings = webView.settings
    settings.javaScriptEnabled = true
    settings.domStorageEnabled = false
    settings.databaseEnabled = false
    settings.cacheMode = WebSettings.LOAD_DEFAULT
    settings.userAgentString = HIDDEN_SOLVER_USER_AGENT
    settings.blockNetworkLoads = true
    webView.setWillNotDraw(true)
    webView.webViewClient = object : WebViewClient() {
      override fun onPageFinished(view: WebView, url: String) {
        super.onPageFinished(view, url)
        synchronized(hiddenSolverLock) {
          if (view !== hiddenSolverWebView) {
            return
          }
          hiddenSolverLoading = false
          hiddenSolverReady = true
          hiddenSolverLock.notifyAll()
        }
      }

      override fun onReceivedHttpError(
        view: WebView,
        request: WebResourceRequest,
        errorResponse: WebResourceResponse,
      ) {
        super.onReceivedHttpError(view, request, errorResponse)
        if (request.isForMainFrame) {
          onHiddenSolverLoadFailed(view)
        }
      }

      override fun onReceivedError(
        view: WebView,
        request: WebResourceRequest,
        error: WebResourceError,
      ) {
        super.onReceivedError(view, request, error)
        if (request.isForMainFrame) {
          onHiddenSolverLoadFailed(view)
        }
      }
    }
  }

  private fun onHiddenSolverLoadFailed(view: WebView) {
    synchronized(hiddenSolverLock) {
      if (view !== hiddenSolverWebView) {
        return
      }
      hiddenSolverLoading = false
      hiddenSolverReady = false
      hiddenSolverLock.notifyAll()
    }
  }

  private fun evaluateHiddenSolver(script: String, timeoutMs: Long): String? {
    val future = CompletableFuture<String?>()
    mainHandler.post {
      val solverWebView = synchronized(hiddenSolverLock) {
        hiddenSolverWebView.takeIf { hiddenSolverReady }
      }
      if (solverWebView == null) {
        future.complete(null)
        return@post
      }
      solverWebView.evaluateJavascript(script) { value ->
        future.complete(value)
      }
    }
    return try {
      future.get(timeoutMs, TimeUnit.MILLISECONDS)
    } catch (_: Exception) {
      future.cancel(true)
      null
    }
  }

  private fun destroyHiddenSolverWebView() {
    if (Looper.myLooper() == Looper.getMainLooper()) {
      destroyHiddenSolverWebViewOnMainThread()
      return
    }
    mainHandler.post { destroyHiddenSolverWebViewOnMainThread() }
  }

  private fun destroyHiddenSolverWebViewOnMainThread() {
    val solverWebView = synchronized(hiddenSolverLock) {
      val view = hiddenSolverWebView
      hiddenSolverWebView = null
      hiddenSolverReady = false
      hiddenSolverLoading = false
      hiddenSolverLock.notifyAll()
      view
    }
    solverWebView?.apply {
      stopLoading()
      destroy()
    }
  }
}
