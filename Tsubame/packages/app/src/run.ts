import type {
  AppHandle,
  AppStartResult,
  Dispose,
  Host,
  HostSession,
  TsubameMount,
} from './host.js';

function isPromise<T>(value: T | Promise<T>): value is Promise<T> {
  return typeof (value as { then?: unknown }).then === 'function';
}

function disposeSafely(dispose: Dispose | undefined): void {
  try {
    dispose?.();
  } catch {
    // Teardown is best-effort so one owner cannot suppress the next cleanup.
  }
}

/** Host lifetime と framework mount を合成する、runtime-blind な Composition Root。 */
export function runTsubameApp(host: Host, mount: TsubameMount): AppHandle {
  let disposed = false;
  let activeSession: HostSession | undefined;
  let mountDispose: Dispose | void;
  let settle!: (result: AppStartResult) => void;
  const settled = new Promise<AppStartResult>((resolve) => {
    settle = resolve;
  });

  const onSession = (session: HostSession): void => {
    if (disposed) {
      disposeSafely(() => session.dispose());
      return;
    }
    activeSession = session;
    try {
      mountDispose = mount(session.renderer);
      settle({ status: 'mounted' });
    } catch (error) {
      activeSession = undefined;
      disposeSafely(() => session.dispose());
      settle({ status: 'failed', error });
    }
  };

  const handle: AppHandle = {
    settled,
    dispose() {
      if (disposed) return;
      disposed = true;
      settle({ status: 'disposed' });
      const session = activeSession;
      activeSession = undefined;
      disposeSafely(typeof mountDispose === 'function' ? mountDispose : undefined);
      mountDispose = undefined;
      disposeSafely(session === undefined ? undefined : () => session.dispose());
    },
  };

  const onStartError = (error: unknown): void => {
    settle({ status: 'failed', error });
  };

  try {
    const started = host.start();
    if (isPromise(started)) {
      void started.then(onSession, onStartError);
    } else {
      onSession(started);
    }
  } catch (error) {
    onStartError(error);
  }

  return handle;
}
