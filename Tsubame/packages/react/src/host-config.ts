import ReactReconciler from 'react-reconciler';
import { DefaultEventPriority, NoEventPriority } from 'react-reconciler/constants';
import { createContext } from 'react';
import type { ElementId, ElementKind, IRenderer } from '@torimi/tsubame-renderer-protocol';
import {
  createInstance as makeInstance,
  type TsubameInstance,
  type TsubameTextInstance,
} from './instance.js';
import { applyInitialProps, applyPropUpdates } from './props.js';

/** reconciler container。生成時に renderer を束縛する（active-renderer グローバルは持たない、ADR-0010）。 */
export interface TsubameContainer {
  readonly renderer: IRenderer;
  readonly rootId: ElementId;
}

type Props = Record<string, unknown>;
type Child = TsubameInstance | TsubameTextInstance;
type HostContext = Record<string, never>;

const noop = (): void => {};

// host context を語彙として使わないが、React は非 null を要求する（null だと
// "Expected host context to exist" 警告になる）。共有の空オブジェクトで足りる。
const HOST_CONTEXT: HostContext = {};

/**
 * 指定 {@link IRenderer} に束縛した `react-reconciler` インスタンスを生成する。
 *
 * mutation モード・write-only。ホスト instance は構造ゼロ（ADR-0010）で、
 * `createInstance` / `appendChild` / `insertBefore` / `removeChild` /
 * `createTextInstance` / `commitTextUpdate` は `IRenderer` をそのまま呼ぶ。
 * 各 instance のリスナ解除は `detachDeletedInstance`（React が削除 instance を
 * 1 つずつ通知する）で行うため、adapter は構造を辿らない。
 */
export function createReconciler(renderer: IRenderer) {
  // React は更新を優先度付きでスケジュールする。現在の更新優先度を保持する
  // （react-three-fiber / ink と同型のボイラープレート）。
  let currentUpdatePriority: number = NoEventPriority;

  return ReactReconciler<
    ElementKind, // Type
    Props, // Props
    TsubameContainer, // Container
    TsubameInstance, // Instance
    TsubameTextInstance, // TextInstance
    never, // SuspenseInstance
    never, // HydratableInstance
    never, // FormInstance
    Child, // PublicInstance
    HostContext, // HostContext
    never, // ChildSet
    ReturnType<typeof setTimeout>, // TimeoutHandle
    -1, // NoTimeout
    null // TransitionStatus
  >({
    supportsMutation: true,
    supportsPersistence: false,
    supportsHydration: false,
    isPrimaryRenderer: true,
    supportsMicrotasks: true,
    scheduleMicrotask: queueMicrotask,

    // --- instance 生成 ---
    createInstance(type, props): TsubameInstance {
      const kind = type as ElementKind;
      const id = renderer.createElement(kind);
      const instance = makeInstance(id, kind);
      applyInitialProps(renderer, instance, props);
      return instance;
    },

    createTextInstance(text): TsubameTextInstance {
      const id = renderer.createElement('text');
      renderer.setText(id, text);
      return { id };
    },

    // text 子は常に独立した `text` element にする（ADR-0058）。要素ごとの
    // テキスト最適化（shouldSetTextContent=true）は使わない。
    shouldSetTextContent(): boolean {
      return false;
    },

    // --- 構造（mutation） ---
    appendInitialChild(parent, child: Child): void {
      renderer.appendChild(parent.id, child.id);
    },
    appendChild(parent, child: Child): void {
      renderer.appendChild(parent.id, child.id);
    },
    appendChildToContainer(container, child: Child): void {
      renderer.appendChild(container.rootId, child.id);
    },
    insertBefore(parent, child: Child, before: Child): void {
      renderer.insertBefore(parent.id, child.id, before.id);
    },
    insertInContainerBefore(container, child: Child, before: Child): void {
      renderer.insertBefore(container.rootId, child.id, before.id);
    },
    removeChild(parent, child: Child): void {
      renderer.removeChild(parent.id, child.id);
    },
    removeChildFromContainer(container, child: Child): void {
      renderer.removeChild(container.rootId, child.id);
    },

    // --- 更新 ---
    commitTextUpdate(textInstance, _oldText, newText): void {
      renderer.setText(textInstance.id, newText);
    },
    commitUpdate(instance, _type, prevProps, nextProps): void {
      applyPropUpdates(renderer, instance, prevProps, nextProps);
    },

    finalizeInitialChildren(): boolean {
      return false;
    },

    // 削除された instance のリスナを解除する。React は削除 subtree の各 instance を
    // ここで 1 つずつ通知するため、構造を辿らずに（shadow tree なしで）掃除できる。
    detachDeletedInstance(node): void {
      for (const unsub of node.listeners.values()) unsub();
      node.listeners.clear();
    },

    // --- host context（語彙を持たないので空オブジェクトで十分） ---
    getRootHostContext: () => HOST_CONTEXT,
    getChildHostContext: () => HOST_CONTEXT,
    getPublicInstance: (instance) => instance,

    // --- commit ライフサイクル ---
    prepareForCommit: () => null,
    resetAfterCommit: noop,
    preparePortalMount: noop,
    clearContainer: noop,
    resetTextContent: noop,

    // --- スケジューラ ---
    scheduleTimeout: setTimeout,
    cancelTimeout: clearTimeout,
    noTimeout: -1,

    // --- 更新優先度 ---
    setCurrentUpdatePriority(priority): void {
      currentUpdatePriority = priority;
    },
    getCurrentUpdatePriority: () => currentUpdatePriority,
    resolveUpdatePriority: () =>
      currentUpdatePriority !== NoEventPriority ? currentUpdatePriority : DefaultEventPriority,

    // --- 使用しない機能のスタブ ---
    getInstanceFromNode: () => null,
    beforeActiveInstanceBlur: noop,
    afterActiveInstanceBlur: noop,
    prepareScopeUpdate: noop,
    getInstanceFromScope: () => null,
    NotPendingTransition: null,
    HostTransitionContext: createContext(null) as never,
    resetFormInstance: noop,
    requestPostPaintCallback: noop,
    shouldAttemptEagerTransition: () => false,
    trackSchedulerEvent: noop,
    resolveEventType: () => null,
    resolveEventTimeStamp: () => -1,
    maySuspendCommit: () => false,
    preloadInstance: () => true,
    startSuspendingCommit: noop,
    suspendInstance: noop,
    waitForCommitToBeReady: () => null,
  });
}

export type TsubameReconciler = ReturnType<typeof createReconciler>;
