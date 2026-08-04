package com.hayateprojects.hayate.adapter_android_demo

import android.content.Context
import android.graphics.Rect
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeProvider
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference
import org.json.JSONObject

/**
 * Android leaf for ADR-0158. Rust owns AccessKit lifecycle, baseline, and action ordering; this
 * object only exposes the latest virtual-view snapshot through [AccessibilityNodeProvider].
 */
object AndroidAccessibilityBridge {
    private const val MOUNT_TIMEOUT_SECONDS = 2L

    private data class VirtualNode(
        val id: Long,
        val parentId: Long?,
        val children: LongArray,
        val className: String,
        val label: String?,
        val value: String?,
        val bounds: Rect,
        val disabled: Boolean,
        val actions: Set<String>,
    )

    private data class Snapshot(
        val rootId: Long,
        val focusId: Long,
        val nodes: Map<Long, VirtualNode>,
    )

    private val snapshot = AtomicReference<Snapshot?>(null)
    private val mountedView = AtomicReference<View?>(null)
    private val activated = AtomicBoolean(false)
    private val containerFocused = AtomicBoolean(false)

    @JvmStatic
    fun mount(): Boolean {
        val activity = CurrentActivity.get() as? MainActivity ?: return false
        val completed = CountDownLatch(1)
        val mounted = AtomicBoolean(false)
        activity.runOnUiThread {
            try {
                val view = activity.findViewById<ViewGroup>(android.R.id.content)
                view.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
                view.isFocusable = true
                view.accessibilityDelegate = object : View.AccessibilityDelegate() {
                    private val provider = HayateNodeProvider(view, activity)

                    override fun getAccessibilityNodeProvider(host: View): AccessibilityNodeProvider =
                        provider
                }
                mountedView.set(view)
                containerFocused.set(view.hasWindowFocus())
                mounted.set(true)
            } finally {
                completed.countDown()
            }
        }
        return completed.await(MOUNT_TIMEOUT_SECONDS, TimeUnit.SECONDS) && mounted.get()
    }

    @JvmStatic
    fun update(snapshotJson: String): Boolean {
        val parsed = parseSnapshot(snapshotJson)
        snapshot.set(parsed)
        val view = mountedView.get() ?: return false
        view.post {
            view.sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)
        }
        return isAssistiveTechnologyActive(view.context) && containerFocused.get()
    }

    @JvmStatic
    fun unmount() {
        snapshot.set(null)
        activated.set(false)
        val view = mountedView.getAndSet(null)
        view?.post {
            view.accessibilityDelegate = null
        }
    }

    fun setContainerFocused(focused: Boolean) {
        containerFocused.set(focused)
        val activity = CurrentActivity.get() as? MainActivity ?: return
        if (!focused) {
            if (activated.getAndSet(false)) activity.accessibilityDeactivate()
            return
        }
        val view = mountedView.get() ?: return
        if (isAssistiveTechnologyActive(view.context) && activated.compareAndSet(false, true)) {
            activity.accessibilityActivate()
            view.sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED)
        }
    }

    private fun ensureActivated(activity: MainActivity, view: View) {
        if (containerFocused.get() && isAssistiveTechnologyActive(view.context) &&
            activated.compareAndSet(false, true)
        ) {
            activity.accessibilityActivate()
        }
    }

    private fun isAssistiveTechnologyActive(context: Context): Boolean {
        val manager = context.getSystemService(AccessibilityManager::class.java)
        return manager?.isEnabled == true && manager.isTouchExplorationEnabled
    }

    private class HayateNodeProvider(
        private val host: View,
        private val activity: MainActivity,
    ) : AccessibilityNodeProvider() {
        override fun createAccessibilityNodeInfo(virtualViewId: Int): AccessibilityNodeInfo? {
            ensureActivated(activity, host)
            val current = snapshot.get()
            if (virtualViewId == View.NO_ID) {
                val info = AccessibilityNodeInfo.obtain()
                info.setSource(host)
                info.packageName = host.context.packageName
                info.className = View::class.java.name
                info.isVisibleToUser = true
                current?.let { info.addChild(host, virtualId(it.rootId)) }
                return info
            }
            val node = current?.nodes?.get(virtualViewId.toLong()) ?: return null
            return nodeInfo(current, node)
        }

        override fun performAction(virtualViewId: Int, action: Int, arguments: Bundle?): Boolean {
            val node = snapshot.get()?.nodes?.get(virtualViewId.toLong()) ?: return false
            val (name, value) = when (action) {
                AccessibilityNodeInfo.ACTION_FOCUS,
                AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS -> "focus" to null
                AccessibilityNodeInfo.ACTION_CLICK -> "click" to null
                AccessibilityNodeInfo.ACTION_SET_TEXT -> "setValue" to
                    arguments?.getCharSequence(
                        AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,
                    )?.toString().orEmpty()
                AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.id ->
                    "scrollIntoView" to null
                else -> return false
            }
            if (name !in node.actions && name != "focus") return false
            activity.accessibilityAction(name, node.id, value)
            return true
        }

        override fun findFocus(focus: Int): AccessibilityNodeInfo? {
            val current = snapshot.get() ?: return null
            return current.nodes[current.focusId]?.let { nodeInfo(current, it) }
        }

        private fun nodeInfo(current: Snapshot, node: VirtualNode): AccessibilityNodeInfo {
            val info = AccessibilityNodeInfo.obtain()
            val id = virtualId(node.id)
            info.setSource(host, id)
            info.packageName = host.context.packageName
            info.className = node.className
            info.text = node.value ?: node.label
            info.contentDescription = node.label
            info.isEnabled = !node.disabled
            info.isVisibleToUser = true
            info.isFocusable = "focus" in node.actions
            info.isFocused = current.focusId == node.id
            info.isClickable = "click" in node.actions
            info.isEditable = "setValue" in node.actions
            info.setBoundsInParent(node.bounds)
            val location = IntArray(2)
            host.getLocationOnScreen(location)
            info.setBoundsInScreen(
                Rect(node.bounds).apply { offset(location[0], location[1]) },
            )
            node.parentId?.let { info.setParent(host, virtualId(it)) } ?: info.setParent(host)
            node.children.forEach { info.addChild(host, virtualId(it)) }
            if ("focus" in node.actions) {
                info.addAction(AccessibilityNodeInfo.ACTION_FOCUS)
                info.addAction(AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS)
            }
            if ("click" in node.actions) info.addAction(AccessibilityNodeInfo.ACTION_CLICK)
            if ("setValue" in node.actions) info.addAction(AccessibilityNodeInfo.ACTION_SET_TEXT)
            if ("scrollIntoView" in node.actions) {
                info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN)
            }
            return info
        }
    }

    private fun virtualId(id: Long): Int = id.toInt()

    private fun parseSnapshot(json: String): Snapshot {
        val root = JSONObject(json)
        val nodes = buildMap {
            val array = root.getJSONArray("nodes")
            for (index in 0 until array.length()) {
                val item = array.getJSONObject(index)
                val id = item.getString("id").toLong()
                val childrenJson = item.getJSONArray("children")
                val children = LongArray(childrenJson.length()) { child ->
                    childrenJson.getString(child).toLong()
                }
                val boundsJson = item.getJSONArray("bounds")
                val actionsJson = item.getJSONArray("actions")
                val actions = buildSet {
                    for (action in 0 until actionsJson.length()) add(actionsJson.getString(action))
                }
                put(
                    id,
                    VirtualNode(
                        id = id,
                        parentId = if (item.isNull("parentId")) {
                            null
                        } else {
                            item.getString("parentId").toLong()
                        },
                        children = children,
                        className = item.getString("className"),
                        label = if (item.isNull("label")) null else item.getString("label"),
                        value = if (item.isNull("value")) null else item.getString("value"),
                        bounds = Rect(
                            boundsJson.getInt(0), boundsJson.getInt(1),
                            boundsJson.getInt(2), boundsJson.getInt(3),
                        ),
                        disabled = item.getBoolean("disabled"),
                        actions = actions,
                    ),
                )
            }
        }
        return Snapshot(
            rootId = root.getString("rootId").toLong(),
            focusId = root.getString("focusId").toLong(),
            nodes = nodes,
        )
    }
}
