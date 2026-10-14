import java.util.Properties
import java.io.File

plugins {
    alias(libs.plugins.androidApplication)
    alias(libs.plugins.composeCompiler)
    alias(libs.plugins.kotlinSerialization)
}

// 兼容级别检测：-Pmicyou.androidCompat=<level>
// 不传参数 → 正常模式（与上游完全一致）
// api21 → Android 5.0+ 兼容模式
// 预留：api19 → Android 4.4+ 兼容模式（未来扩展）
val compatLevel = providers.gradleProperty("micyou.androidCompat").orNull?.takeIf { it.isNotBlank() }
val androidCompat = compatLevel != null
val isApi21Compat = compatLevel == "api21"
// 预留更低级别，当前未实现
// val isApi19Compat = compatLevel == "api19"

if (androidCompat) {
    apply(plugin = "org.jetbrains.kotlin.android")
}

val aifadianApiToken: String = run {
    val localProps = Properties()
    val localFile = File(rootDir, "local.properties")
    if (localFile.exists()) {
        localFile.inputStream().use { localProps.load(it) }
    }
    localProps.getProperty("AIFADIAN_API_TOKEN", "")
}

val aifadianUserId: String = run {
    val localProps = Properties()
    val localFile = File(rootDir, "local.properties")
    if (localFile.exists()) {
        localFile.inputStream().use { localProps.load(it) }
    }
    localProps.getProperty("AIFADIAN_USER_ID", "")
}

android {
    namespace = "com.lanrhyme.micyou"
    compileSdk = libs.versions.android.compileSdk.get().toInt()

    defaultConfig {
        applicationId = "com.lanrhyme.micyou"
        minSdk = (if (androidCompat) libs.versions.compat.android.minSdk.get() else libs.versions.android.minSdk.get()).toInt()
        targetSdk = (if (androidCompat) libs.versions.compat.android.targetSdk.get() else libs.versions.android.targetSdk.get()).toInt()
        versionCode = project.property("project.version.code").toString().toInt()
        versionName = project.property("project.version").toString()
        buildConfigField("String", "AIFADIAN_API_TOKEN", "\"$aifadianApiToken\"")
        buildConfigField("String", "AIFADIAN_USER_ID", "\"$aifadianUserId\"")

        if (androidCompat) {
            // 兼容模式：显式启用 MultiDex
            // Android 5.0 (API 21) 虽宣称 ART 原生支持 MultiDex，但部分厂商设备 (如华为 P7 API 22)
            // 存在 DEX 加载 bug：第二个 DEX 中的合成类会抛 NoClassDefFoundError。
            multiDexEnabled = true

            // 32位架构支持
            ndk {
                abiFilters += listOf("armeabi-v7a", "x86")
            }
        }
    }

    buildFeatures {
        buildConfig = true
        compose = true
    }

    val keystorePath = providers.environmentVariable("ANDROID_KEYSTORE_PATH").orNull
    val keystorePassword = providers.environmentVariable("ANDROID_KEYSTORE_PASSWORD").orNull
    val keyAlias = providers.environmentVariable("ANDROID_KEY_ALIAS").orNull
    val keyPassword = providers.environmentVariable("ANDROID_KEY_PASSWORD").orNull

    val hasReleaseSigning =
        !keystorePath.isNullOrEmpty() &&
        !keystorePassword.isNullOrEmpty() &&
        !keyAlias.isNullOrEmpty() &&
        !keyPassword.isNullOrEmpty()

    signingConfigs {
        create("release") {
            if (hasReleaseSigning) {
                storeFile = file(keystorePath!!)
                storePassword = keystorePassword
                this.keyAlias = keyAlias
                this.keyPassword = keyPassword
            }
        }
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
    buildTypes {
        getByName("release") {
            isMinifyEnabled = if (androidCompat) false else true
            isShrinkResources = if (androidCompat) false else true
            if (!androidCompat) {
                proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
            }

            if (hasReleaseSigning) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }
    compileOptions {
        sourceCompatibility = if (androidCompat) JavaVersion.VERSION_1_8 else JavaVersion.VERSION_11
        targetCompatibility = if (androidCompat) JavaVersion.VERSION_1_8 else JavaVersion.VERSION_11
        if (androidCompat) {
            // 兼容模式：启用核心库脱糖 (core library desugaring)
            // 让低版本 Android 设备可用 Java 8+ 标准库 API
            isCoreLibraryDesugaringEnabled = true
        }
    }

    lint {
        checkReleaseBuilds = false
        abortOnError = false
        disable += listOf("ExpiredTargetSdkVersion")
    }

    if (androidCompat) {
        // 兼容模式：使用 compat 源集的 haze 兼容实现
        sourceSets {
            getByName("main") {
                kotlin.srcDir("src/compat/kotlin")
            }
        }
    } else {
        // 正常模式：使用 normal 源集的 haze 桥接实现
        sourceSets {
            getByName("main") {
                kotlin.srcDir("src/normal/kotlin")
            }
        }
    }
}

// Kotlin 2.3+ 使用 compilerOptions DSL 替代已废弃的 kotlinOptions
// 兼容模式经 apply() 引入 kotlin-android，没有生成 kotlin {} 访问器，按类型配置扩展
extensions.configure<org.jetbrains.kotlin.gradle.dsl.KotlinAndroidProjectExtension> {
    compilerOptions {
        jvmTarget.set(
            if (androidCompat) org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_1_8
            else org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_11
        )
    }
}

// 兼容模式：强制降级依赖版本以支持 minSdk 21
if (androidCompat) {
    configurations.all {
        resolutionStrategy {
            force("androidx.activity:activity:1.8.2")
            force("androidx.activity:activity-compose:1.8.2")
            force("androidx.activity:activity-ktx:1.8.2")
            force("androidx.core:core-ktx:${libs.versions.compat.androidx.core.get()}")
            force("androidx.lifecycle:lifecycle-viewmodel-compose:${libs.versions.compat.androidx.lifecycle.get()}")
            force("androidx.lifecycle:lifecycle-runtime-compose:${libs.versions.compat.androidx.lifecycle.get()}")
            force("io.ktor:ktor-network:${libs.versions.compat.ktor.get()}")
            force("io.ktor:ktor-client-core:${libs.versions.compat.ktor.get()}")
            force("io.ktor:ktor-client-okhttp:${libs.versions.compat.ktor.get()}")
            force("io.ktor:ktor-client-content-negotiation:${libs.versions.compat.ktor.get()}")
            force("io.ktor:ktor-serialization-kotlinx-json:${libs.versions.compat.ktor.get()}")
            force("org.jetbrains.kotlinx:kotlinx-coroutines-android:${libs.versions.compat.kotlinx.coroutines.get()}")
            force("org.jetbrains.kotlinx:kotlinx-coroutines-core:${libs.versions.compat.kotlinx.coroutines.get()}")
            force("org.jetbrains.kotlinx:kotlinx-serialization-core:${libs.versions.compat.kotlinx.serialization.get()}")
            force("org.jetbrains.kotlinx:kotlinx-serialization-protobuf:${libs.versions.compat.kotlinx.serialization.get()}")
            force("org.jetbrains.kotlinx:kotlinx-serialization-json:${libs.versions.compat.kotlinx.serialization.get()}")
            force("org.jetbrains.kotlinx:kotlinx-datetime:${libs.versions.compat.kotlinx.datetime.get()}")
            force("com.materialkolor:material-kolor:${libs.versions.compat.materialKolor.get()}")
            // Compose BOM 降级
            force("androidx.compose:compose-bom:${libs.versions.compat.compose.bom.get()}")
            // Compose 库降级：force() 对传递依赖不生效，用 eachDependency 统一覆盖所有 Compose 依赖
            eachDependency {
                if (requested.group?.startsWith("androidx.compose") == true) {
                    // 跳过 BOM（版本格式不同）和不存在的库
                    if (requested.name == "compose-bom") return@eachDependency
                    if (requested.name in setOf("runtime-retain", "runtime-annotation", "runtime-tracing")) return@eachDependency
                    when {
                        requested.group == "androidx.compose.material3" -> useVersion("1.3.0")
                        else -> useVersion("1.7.1")
                    }
                }
            }
        }
    }
}

dependencies {
    if (androidCompat) {
        // 兼容模式：MultiDex 库 + core library desugaring
        implementation("androidx.multidex:multidex:${libs.versions.compat.multidex.get()}")
        coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:${libs.versions.compat.desugarJdkLibs.get()}")
    }

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.viewmodelCompose)
    implementation(libs.androidx.lifecycle.runtimeCompose)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.material.icons.extended)
    implementation(libs.ktor.network)
    implementation(libs.ktor.client.core)
    implementation(libs.ktor.client.okhttp)
    implementation(libs.ktor.client.content.negotiation)
    implementation(libs.ktor.serialization.kotlinx.json)
    implementation(libs.kotlinx.serialization.protobuf)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.datetime)
    implementation(libs.haze) {
        if (androidCompat) {
            exclude(group = "org.jetbrains.compose.ui")
            exclude(group = "org.jetbrains.compose.runtime")
            exclude(group = "org.jetbrains.compose.foundation")
            exclude(group = "org.jetbrains.compose.material")
            exclude(group = "org.jetbrains.compose.animation")
        }
    }
    // haze-blur only exists from Haze 2; the compat bridge draws no blur.
    if (!androidCompat) {
        implementation(libs.haze.blur)
    }
    implementation(libs.materialKolor) {
        if (androidCompat) {
            exclude(group = "org.jetbrains.compose.ui")
            exclude(group = "org.jetbrains.compose.runtime")
            exclude(group = "org.jetbrains.compose.foundation")
            exclude(group = "org.jetbrains.compose.material")
            exclude(group = "org.jetbrains.compose.animation")
        }
    }
    implementation(libs.concentus)

    debugImplementation(libs.androidx.compose.ui.tooling)
}
