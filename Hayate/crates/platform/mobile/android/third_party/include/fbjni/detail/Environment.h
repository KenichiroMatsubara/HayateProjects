/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#pragma once

#include <jni.h>

#include <functional>
#include <string>

namespace facebook {
namespace jni {

struct Environment {
  static JNIEnv* current();
  static void initialize(JavaVM* vm);
  static JNIEnv* ensureCurrentThreadIsAttached();
  static bool isGlobalJvmAvailable();
};

namespace detail {

JNIEnv* currentOrNull();
JNIEnv* cachedWithAttachmentState(bool& isAttaching);

struct TLData {
  JNIEnv* env;
  bool attached;
};

class JniEnvCacher {
 public:
  JniEnvCacher(JNIEnv* env);
  JniEnvCacher(JniEnvCacher&) = delete;
  JniEnvCacher(JniEnvCacher&&) = default;
  JniEnvCacher& operator=(JniEnvCacher&) = delete;
  JniEnvCacher& operator=(JniEnvCacher&&) = delete;
  ~JniEnvCacher();

 private:
  bool thisCached_;
  detail::TLData data_;
};

}  // namespace detail

class ThreadScope {
 public:
  ThreadScope();
  ThreadScope(ThreadScope&) = delete;
  ThreadScope(ThreadScope&&) = default;
  ThreadScope& operator=(ThreadScope&) = delete;
  ThreadScope& operator=(ThreadScope&&) = delete;
  ~ThreadScope();

  static void WithClassLoader(std::function<void()>&& runnable);
  static void OnLoad();

 private:
  bool thisAttached_;
  detail::TLData data_;
};

}  // namespace jni
}  // namespace facebook
