plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}
android {
    namespace = "dev.synchrogit.app"
    compileSdk = 36
    defaultConfig {
        applicationId = "dev.synchrogit.app"
        minSdk = 26
        targetSdk = 36
        versionCode = providers.environmentVariable("ANDROID_VERSION_CODE").orElse("26090001").get().toInt()
        versionName = providers.environmentVariable("SYNCHROGIT_VERSION").orElse("26.9.0-dev").get()
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    signingConfigs {
        create("release") {
            storeFile = providers.environmentVariable("ANDROID_KEYSTORE_PATH").orNull?.let { file(it) }
            storePassword = providers.environmentVariable("ANDROID_KEYSTORE_PASSWORD").orNull
            keyAlias = "synchrogit"
            keyPassword = providers.environmentVariable("ANDROID_KEYSTORE_PASSWORD").orNull
        }
    }
    buildTypes {
        release { signingConfig = signingConfigs.getByName("release"); isMinifyEnabled = false }
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { compose = true }
}
dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.10.01"))
    implementation("androidx.activity:activity-compose:1.11.0")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.9.4")
    implementation("androidx.work:work-runtime-ktx:2.10.5")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test:rules:1.7.0")
    androidTestImplementation(platform("androidx.compose:compose-bom:2025.10.01"))
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
