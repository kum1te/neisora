/// <reference types="vite/client" />
import { invoke as nativeInvoke } from '@tauri-apps/api/core';

// The fixture hook is compiled out of normal builds. Test builds exercise the same
// components and event listeners without generating fake listening signals online.
export const invoke: typeof nativeInvoke = (command, args, options) => {
  if (import.meta.env.MODE === 'test') {
    const fixture = (window as unknown as { __VESPER_TEST_INVOKE__?: typeof nativeInvoke }).__VESPER_TEST_INVOKE__;
    if (fixture) return fixture(command, args, options);
  }
  return nativeInvoke(command, args, options);
};
