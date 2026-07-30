# Add project specific ProGuard rules here.
-keepattributes SourceFile,LineNumberTable

# Keep all Rust/WebView exposed methods
-keep class com.gullbur.enclave.** { *; }
-keep class app.tauri.** { *; }

# Keep Kotlin serialization
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt

# Keep JNI native methods
-keepclasseswithmembernames class * {
    native <methods>;
}

# Remove logging in release
-assumenosideeffects class android.util.Log {
    public static boolean isLoggable(java.lang.String, int);
    public static int v(...);
    public static int d(...);
    public static int i(...);
}