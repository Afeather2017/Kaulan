//! Android wake lock implementation for keeping the CPU awake during playback.
//!
//! Uses JNI to call Android's `PowerManager.newWakeLock()` API directly.
//! Only compiled for Android targets.

#[cfg(target_os = "android")]
use jni::{
    objects::{GlobalRef, JObject, JValue},
    JavaVM,
};

/// Partial wake lock level: keeps CPU running, allows screen/keyboard to go off.
#[cfg(target_os = "android")]
const PARTIAL_WAKE_LOCK: i32 = 0x00000001;

/// A partial wake lock that keeps the CPU running.
///
/// Acquired on server startup and released on server stop to ensure
/// the backend stays responsive for music streaming.
#[cfg(target_os = "android")]
pub struct WakeLock {
    inner: GlobalRef,
    vm: JavaVM,
    tag: String,
    acquired: bool,
}

#[cfg(target_os = "android")]
impl WakeLock {
    /// Create a new partial wake lock with the given tag.
    pub fn new(tag: &str) -> Result<Self, String> {
        let ctx = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(ctx.vm() as *mut _) }
            .map_err(|e| format!("Failed to get JavaVM: {}", e))?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| format!("Failed to attach thread: {}", e))?;

        // Fetch the PowerManager system service.
        let service_name = env
            .new_string("power")
            .map_err(|e| format!("Failed to create string: {}", e))?;

        let power_manager = catch_exception(&mut env, |env| {
            env.call_method(
                unsafe { JObject::from_raw(ctx.context().cast()) },
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::from(&service_name)],
            )?
            .l()
        })?;

        let tag_str = env
            .new_string(tag)
            .map_err(|e| format!("Failed to create tag string: {}", e))?;

        // Create the wake lock.
        let result = catch_exception(&mut env, |env| {
            env.call_method(
                &power_manager,
                "newWakeLock",
                "(ILjava/lang/String;)Landroid/os/PowerManager$WakeLock;",
                &[
                    JValue::from(PARTIAL_WAKE_LOCK),
                    JValue::from(&tag_str),
                ],
            )
        })?;

        let wake_lock = env
            .new_global_ref(result.l().map_err(|e| e.to_string())?)
            .map_err(|e| format!("Failed to create global ref: {}", e))?;

        // Drop env before moving vm into the struct (env borrows vm).
        drop(env);

        Ok(Self {
            inner: wake_lock,
            vm,
            tag: tag.to_string(),
            acquired: false,
        })
    }

    /// Acquire the wake lock.
    pub fn acquire(&mut self) -> Result<(), String> {
        if self.acquired {
            return Ok(());
        }
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| format!("Failed to attach thread: {}", e))?;
        catch_exception(&mut env, |env| {
            env.call_method(&self.inner, "acquire", "()V", &[])
        })?;
        self.acquired = true;
        log::info!("Acquired wake lock \"{}\"", self.tag);
        Ok(())
    }

    /// Release the wake lock.
    pub fn release(&mut self) -> Result<(), String> {
        if !self.acquired {
            return Ok(());
        }
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| format!("Failed to attach thread: {}", e))?;
        catch_exception(&mut env, |env| {
            env.call_method(&self.inner, "release", "()V", &[])
        })?;
        self.acquired = false;
        log::info!("Released wake lock \"{}\"", self.tag);
        Ok(())
    }
}

#[cfg(target_os = "android")]
impl Drop for WakeLock {
    fn drop(&mut self) {
        if let Err(e) = self.release() {
            log::error!("Error releasing wake lock on drop: {}", e);
        }
    }
}

/// Helper to catch Java exceptions and convert them to formatted Rust errors.
#[cfg(target_os = "android")]
fn catch_exception<'a, T, F>(env: &mut jni::JNIEnv<'a>, f: F) -> Result<T, String>
where
    F: FnOnce(&mut jni::JNIEnv<'a>) -> jni::errors::Result<T>,
{
    match f(env) {
        Ok(value) => Ok(value),
        Err(e @ jni::errors::Error::JavaException) => {
            let message = if let Ok(exception) = env.exception_occurred() {
                let _ = env.exception_clear();
                env.call_method(exception, "getMessage", "()Ljava/lang/String;", &[])
                    .and_then(|value| value.l())
                    .and_then(|msg| {
                        env.get_string(&msg.into())
                            .map(|s| s.to_string_lossy().into_owned())
                    })
                    .ok()
            } else {
                None
            };
            Err(message.unwrap_or_else(|| e.to_string()))
        }
        Err(e) => Err(e.to_string()),
    }
}
