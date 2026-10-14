package com.lanrhyme.micyou

import android.app.Application
import android.content.Context
import android.os.Build
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * 全局 Application 类
 * - 反射安装 MultiDex（无依赖时静默跳过，不影响原版编译）
 * - 设置全局未捕获异常处理器，崩溃日志记录到外部存储 crash 目录
 * - 兼容 Android 5.0+ (API 21+)
 */
class MicYouApplication : Application() {

    override fun attachBaseContext(base: Context?) {
        super.attachBaseContext(base)
        // 通过反射安装 MultiDex：兼容模式下 multidex 库存在时生效
        // 原版构建无此依赖时静默跳过，不影响运行
        try {
            val multiDexClass = Class.forName("androidx.multidex.MultiDex")
            val installMethod = multiDexClass.getMethod("install", Application::class.java)
            installMethod.invoke(null, this)
        } catch (_: Throwable) {
            // 无 MultiDex 依赖或安装失败时静默忽略
            // 原版构建：直接跳过，不影响功能
            // 兼容模式：退化为系统默认 MultiDex 行为
        }
    }

    override fun onCreate() {
        super.onCreate()
        // 安装全局未捕获异常处理器
        val previousHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            try {
                // 记录崩溃日志到文件
                val crashDir = File(getExternalFilesDir(null), "crash")
                if (!crashDir.exists()) {
                    crashDir.mkdirs()
                }
                val timestamp = SimpleDateFormat("yyyy-MM-dd_HH-mm-ss", Locale.getDefault()).format(Date())
                val logFile = File(crashDir, "crash_${timestamp}.txt")
                val sw = StringWriter()
                val pw = PrintWriter(sw)
                pw.println("=== MicYou Crash Report ===")
                pw.println("Time: ${Date()}")
                pw.println("Thread: ${thread.name}")
                pw.println("Device: ${Build.MANUFACTURER} ${Build.MODEL}")
                pw.println("Android API: ${Build.VERSION.SDK_INT}")
                pw.println("App Version: ${packageManager.getPackageInfo(packageName, 0).versionName}")
                pw.println("============================")
                pw.println()
                throwable.printStackTrace(pw)
                pw.println()
                pw.println("=== Thread Stack ===")
                thread.stackTrace.forEach { pw.println(it.toString()) }
                pw.flush()
                logFile.writeText(sw.toString())
            } catch (_: Exception) {
                // 如果连日志都写不了，至少不要吞掉原始异常
            }

            // 交给系统默认处理器终止进程
            previousHandler?.uncaughtException(thread, throwable)
        }
    }
}
