(function() {
	//#region ../../../node_modules/.pnpm/solid-js@1.9.13/node_modules/solid-js/dist/solid.js
	var sharedConfig = {
		context: void 0,
		registry: void 0,
		effects: void 0,
		done: false,
		getContextId() {
			return getContextId(this.context.count);
		},
		getNextContextId() {
			return getContextId(this.context.count++);
		}
	};
	function getContextId(count) {
		const num = String(count), len = num.length - 1;
		return sharedConfig.context.id + (len ? String.fromCharCode(96 + len) : "") + num;
	}
	function setHydrateContext(context) {
		sharedConfig.context = context;
	}
	function nextHydrateContext() {
		return {
			...sharedConfig.context,
			id: sharedConfig.getNextContextId(),
			count: 0
		};
	}
	var equalFn = (a, b) => a === b;
	var $PROXY = Symbol("solid-proxy");
	var SUPPORTS_PROXY = typeof Proxy === "function";
	var signalOptions = { equals: equalFn };
	var ERROR = null;
	var runEffects = runQueue;
	var STALE = 1;
	var PENDING = 2;
	var UNOWNED = {
		owned: null,
		cleanups: null,
		context: null,
		owner: null
	};
	var Owner = null;
	var Transition = null;
	var Scheduler = null;
	var ExternalSourceConfig = null;
	var Listener = null;
	var Updates = null;
	var Effects = null;
	var ExecCount = 0;
	function createRoot(fn, detachedOwner) {
		const listener = Listener, owner = Owner, unowned = fn.length === 0, current = detachedOwner === void 0 ? owner : detachedOwner, root = unowned ? UNOWNED : {
			owned: null,
			cleanups: null,
			context: current ? current.context : null,
			owner: current
		}, updateFn = unowned ? fn : () => fn(() => untrack(() => cleanNode(root)));
		Owner = root;
		Listener = null;
		try {
			return runUpdates(updateFn, true);
		} finally {
			Listener = listener;
			Owner = owner;
		}
	}
	function createSignal(value, options) {
		options = options ? Object.assign({}, signalOptions, options) : signalOptions;
		const s = {
			value,
			observers: null,
			observerSlots: null,
			comparator: options.equals || void 0
		};
		const setter = (value) => {
			if (typeof value === "function") if (Transition && Transition.running && Transition.sources.has(s)) value = value(s.tValue);
			else value = value(s.value);
			return writeSignal(s, value);
		};
		return [readSignal.bind(s), setter];
	}
	function createRenderEffect(fn, value, options) {
		const c = createComputation(fn, value, false, STALE);
		if (Scheduler && Transition && Transition.running) Updates.push(c);
		else updateComputation(c);
	}
	function createEffect(fn, value, options) {
		runEffects = runUserEffects;
		const c = createComputation(fn, value, false, STALE), s = SuspenseContext && useContext(SuspenseContext);
		if (s) c.suspense = s;
		if (!options || !options.render) c.user = true;
		Effects ? Effects.push(c) : updateComputation(c);
	}
	function createMemo(fn, value, options) {
		options = options ? Object.assign({}, signalOptions, options) : signalOptions;
		const c = createComputation(fn, value, true, 0);
		c.observers = null;
		c.observerSlots = null;
		c.comparator = options.equals || void 0;
		if (Scheduler && Transition && Transition.running) {
			c.tState = STALE;
			Updates.push(c);
		} else updateComputation(c);
		return readSignal.bind(c);
	}
	function untrack(fn) {
		if (!ExternalSourceConfig && Listener === null) return fn();
		const listener = Listener;
		Listener = null;
		try {
			if (ExternalSourceConfig) return ExternalSourceConfig.untrack(fn);
			return fn();
		} finally {
			Listener = listener;
		}
	}
	function onCleanup(fn) {
		if (Owner === null);
		else if (Owner.cleanups === null) Owner.cleanups = [fn];
		else Owner.cleanups.push(fn);
		return fn;
	}
	function startTransition(fn) {
		if (Transition && Transition.running) {
			fn();
			return Transition.done;
		}
		const l = Listener;
		const o = Owner;
		return Promise.resolve().then(() => {
			Listener = l;
			Owner = o;
			let t;
			if (Scheduler || SuspenseContext) {
				t = Transition || (Transition = {
					sources: /* @__PURE__ */ new Set(),
					effects: [],
					promises: /* @__PURE__ */ new Set(),
					disposed: /* @__PURE__ */ new Set(),
					queue: /* @__PURE__ */ new Set(),
					running: true
				});
				t.done || (t.done = new Promise((res) => t.resolve = res));
				t.running = true;
			}
			runUpdates(fn, false);
			Listener = Owner = null;
			return t ? t.done : void 0;
		});
	}
	var [transPending, setTransPending] = /* @__PURE__ */ createSignal(false);
	function useContext(context) {
		let value;
		return Owner && Owner.context && (value = Owner.context[context.id]) !== void 0 ? value : context.defaultValue;
	}
	var SuspenseContext;
	function readSignal() {
		const runningTransition = Transition && Transition.running;
		if (this.sources && (runningTransition ? this.tState : this.state)) if ((runningTransition ? this.tState : this.state) === STALE) updateComputation(this);
		else {
			const updates = Updates;
			Updates = null;
			runUpdates(() => lookUpstream(this), false);
			Updates = updates;
		}
		if (Listener) {
			const observers = this.observers;
			if (!observers || observers[observers.length - 1] !== Listener) {
				const sSlot = observers ? observers.length : 0;
				if (!Listener.sources) {
					Listener.sources = [this];
					Listener.sourceSlots = [sSlot];
				} else {
					Listener.sources.push(this);
					Listener.sourceSlots.push(sSlot);
				}
				if (!observers) {
					this.observers = [Listener];
					this.observerSlots = [Listener.sources.length - 1];
				} else {
					observers.push(Listener);
					this.observerSlots.push(Listener.sources.length - 1);
				}
			}
		}
		if (runningTransition && Transition.sources.has(this)) return this.tValue;
		return this.value;
	}
	function writeSignal(node, value, isComp) {
		let current = Transition && Transition.running && Transition.sources.has(node) ? node.tValue : node.value;
		if (!node.comparator || !node.comparator(current, value)) {
			if (Transition) {
				const TransitionRunning = Transition.running;
				if (TransitionRunning || !isComp && Transition.sources.has(node)) {
					Transition.sources.add(node);
					node.tValue = value;
				}
				if (!TransitionRunning) node.value = value;
			} else node.value = value;
			if (node.observers && node.observers.length) runUpdates(() => {
				for (let i = 0; i < node.observers.length; i += 1) {
					const o = node.observers[i];
					const TransitionRunning = Transition && Transition.running;
					if (TransitionRunning && Transition.disposed.has(o)) continue;
					if (TransitionRunning ? !o.tState : !o.state) {
						if (o.pure) Updates.push(o);
						else Effects.push(o);
						if (o.observers) markDownstream(o);
					}
					if (!TransitionRunning) o.state = STALE;
					else o.tState = STALE;
				}
				if (Updates.length > 1e6) {
					Updates = [];
					throw new Error();
				}
			}, false);
		}
		return value;
	}
	function updateComputation(node) {
		if (!node.fn) return;
		cleanNode(node);
		const time = ExecCount;
		runComputation(node, Transition && Transition.running && Transition.sources.has(node) ? node.tValue : node.value, time);
		if (Transition && !Transition.running && Transition.sources.has(node)) queueMicrotask(() => {
			runUpdates(() => {
				Transition && (Transition.running = true);
				Listener = Owner = node;
				runComputation(node, node.tValue, time);
				Listener = Owner = null;
			}, false);
		});
	}
	function runComputation(node, value, time) {
		let nextValue;
		const owner = Owner, listener = Listener;
		Listener = Owner = node;
		try {
			nextValue = node.fn(value);
		} catch (err) {
			if (node.pure) if (Transition && Transition.running) {
				node.tState = STALE;
				node.tOwned && node.tOwned.forEach(cleanNode);
				node.tOwned = void 0;
			} else {
				node.state = STALE;
				node.owned && node.owned.forEach(cleanNode);
				node.owned = null;
			}
			node.updatedAt = time + 1;
			return handleError(err);
		} finally {
			Listener = listener;
			Owner = owner;
		}
		if (!node.updatedAt || node.updatedAt <= time) {
			if (node.updatedAt != null && "observers" in node) writeSignal(node, nextValue, true);
			else if (Transition && Transition.running && node.pure) {
				if (!Transition.sources.has(node)) node.value = nextValue;
				Transition.sources.add(node);
				node.tValue = nextValue;
			} else node.value = nextValue;
			node.updatedAt = time;
		}
	}
	function createComputation(fn, init, pure, state = STALE, options) {
		const c = {
			fn,
			state,
			updatedAt: null,
			owned: null,
			sources: null,
			sourceSlots: null,
			cleanups: null,
			value: init,
			owner: Owner,
			context: Owner ? Owner.context : null,
			pure
		};
		if (Transition && Transition.running) {
			c.state = 0;
			c.tState = state;
		}
		if (Owner === null);
		else if (Owner !== UNOWNED) if (Transition && Transition.running && Owner.pure) if (!Owner.tOwned) Owner.tOwned = [c];
		else Owner.tOwned.push(c);
		else if (!Owner.owned) Owner.owned = [c];
		else Owner.owned.push(c);
		if (ExternalSourceConfig && c.fn) {
			const sourceFn = c.fn;
			const [track, trigger] = createSignal(void 0, { equals: false });
			const ordinary = ExternalSourceConfig.factory(sourceFn, trigger);
			onCleanup(() => ordinary.dispose());
			let inTransition;
			const triggerInTransition = () => startTransition(trigger).then(() => {
				if (inTransition) {
					inTransition.dispose();
					inTransition = void 0;
				}
			});
			c.fn = (x) => {
				track();
				if (Transition && Transition.running) {
					if (!inTransition) inTransition = ExternalSourceConfig.factory(sourceFn, triggerInTransition);
					return inTransition.track(x);
				}
				return ordinary.track(x);
			};
		}
		return c;
	}
	function runTop(node) {
		const runningTransition = Transition && Transition.running;
		if ((runningTransition ? node.tState : node.state) === 0) return;
		if ((runningTransition ? node.tState : node.state) === PENDING) return lookUpstream(node);
		if (node.suspense && untrack(node.suspense.inFallback)) return node.suspense.effects.push(node);
		const ancestors = [node];
		while ((node = node.owner) && (!node.updatedAt || node.updatedAt < ExecCount)) {
			if (runningTransition && Transition.disposed.has(node)) return;
			if (runningTransition ? node.tState : node.state) ancestors.push(node);
		}
		for (let i = ancestors.length - 1; i >= 0; i--) {
			node = ancestors[i];
			if (runningTransition) {
				let top = node, prev = ancestors[i + 1];
				while ((top = top.owner) && top !== prev) if (Transition.disposed.has(top)) return;
			}
			if ((runningTransition ? node.tState : node.state) === STALE) updateComputation(node);
			else if ((runningTransition ? node.tState : node.state) === PENDING) {
				const updates = Updates;
				Updates = null;
				runUpdates(() => lookUpstream(node, ancestors[0]), false);
				Updates = updates;
			}
		}
	}
	function runUpdates(fn, init) {
		if (Updates) return fn();
		let wait = false;
		if (!init) Updates = [];
		if (Effects) wait = true;
		else Effects = [];
		ExecCount++;
		try {
			const res = fn();
			completeUpdates(wait);
			return res;
		} catch (err) {
			if (!wait) Effects = null;
			Updates = null;
			handleError(err);
		}
	}
	function completeUpdates(wait) {
		if (Updates) {
			if (Scheduler && Transition && Transition.running) scheduleQueue(Updates);
			else runQueue(Updates);
			Updates = null;
		}
		if (wait) return;
		let res;
		if (Transition) {
			if (!Transition.promises.size && !Transition.queue.size) {
				const sources = Transition.sources;
				const disposed = Transition.disposed;
				Effects.push.apply(Effects, Transition.effects);
				res = Transition.resolve;
				for (const e of Effects) {
					"tState" in e && (e.state = e.tState);
					delete e.tState;
				}
				Transition = null;
				runUpdates(() => {
					for (const d of disposed) cleanNode(d);
					for (const v of sources) {
						v.value = v.tValue;
						if (v.owned) for (let i = 0, len = v.owned.length; i < len; i++) cleanNode(v.owned[i]);
						if (v.tOwned) v.owned = v.tOwned;
						delete v.tValue;
						delete v.tOwned;
						v.tState = 0;
					}
					setTransPending(false);
				}, false);
			} else if (Transition.running) {
				Transition.running = false;
				Transition.effects.push.apply(Transition.effects, Effects);
				Effects = null;
				setTransPending(true);
				return;
			}
		}
		const e = Effects;
		Effects = null;
		if (e.length) runUpdates(() => runEffects(e), false);
		if (res) res();
	}
	function runQueue(queue) {
		for (let i = 0; i < queue.length; i++) runTop(queue[i]);
	}
	function scheduleQueue(queue) {
		for (let i = 0; i < queue.length; i++) {
			const item = queue[i];
			const tasks = Transition.queue;
			if (!tasks.has(item)) {
				tasks.add(item);
				Scheduler(() => {
					tasks.delete(item);
					runUpdates(() => {
						Transition.running = true;
						runTop(item);
					}, false);
					Transition && (Transition.running = false);
				});
			}
		}
	}
	function runUserEffects(queue) {
		let i, userLength = 0;
		for (i = 0; i < queue.length; i++) {
			const e = queue[i];
			if (!e.user) runTop(e);
			else queue[userLength++] = e;
		}
		if (sharedConfig.context) {
			if (sharedConfig.count) {
				sharedConfig.effects || (sharedConfig.effects = []);
				sharedConfig.effects.push(...queue.slice(0, userLength));
				return;
			}
			setHydrateContext();
		}
		if (sharedConfig.effects && (sharedConfig.done || !sharedConfig.count)) {
			queue = [...sharedConfig.effects, ...queue];
			userLength += sharedConfig.effects.length;
			delete sharedConfig.effects;
		}
		for (i = 0; i < userLength; i++) runTop(queue[i]);
	}
	function lookUpstream(node, ignore) {
		const runningTransition = Transition && Transition.running;
		if (runningTransition) node.tState = 0;
		else node.state = 0;
		for (let i = 0; i < node.sources.length; i += 1) {
			const source = node.sources[i];
			if (source.sources) {
				const state = runningTransition ? source.tState : source.state;
				if (state === STALE) {
					if (source !== ignore && (!source.updatedAt || source.updatedAt < ExecCount)) runTop(source);
				} else if (state === PENDING) lookUpstream(source, ignore);
			}
		}
	}
	function markDownstream(node) {
		const runningTransition = Transition && Transition.running;
		for (let i = 0; i < node.observers.length; i += 1) {
			const o = node.observers[i];
			if (runningTransition ? !o.tState : !o.state) {
				if (runningTransition) o.tState = PENDING;
				else o.state = PENDING;
				if (o.pure) Updates.push(o);
				else Effects.push(o);
				o.observers && markDownstream(o);
			}
		}
	}
	function cleanNode(node) {
		let i;
		if (node.sources) while (node.sources.length) {
			const source = node.sources.pop(), index = node.sourceSlots.pop(), obs = source.observers;
			if (obs && obs.length) {
				const n = obs.pop(), s = source.observerSlots.pop();
				if (index < obs.length) {
					n.sourceSlots[s] = index;
					obs[index] = n;
					source.observerSlots[index] = s;
				}
			}
		}
		if (node.tOwned) {
			for (i = node.tOwned.length - 1; i >= 0; i--) cleanNode(node.tOwned[i]);
			delete node.tOwned;
		}
		if (Transition && Transition.running && node.pure) reset(node, true);
		else if (node.owned) {
			for (i = node.owned.length - 1; i >= 0; i--) cleanNode(node.owned[i]);
			node.owned = null;
		}
		if (node.cleanups) {
			for (i = node.cleanups.length - 1; i >= 0; i--) node.cleanups[i]();
			node.cleanups = null;
		}
		if (Transition && Transition.running) node.tState = 0;
		else node.state = 0;
	}
	function reset(node, top) {
		if (!top) {
			node.tState = 0;
			Transition.disposed.add(node);
		}
		if (node.owned) for (let i = 0; i < node.owned.length; i++) reset(node.owned[i]);
	}
	function castError(err) {
		if (err instanceof Error) return err;
		return new Error(typeof err === "string" ? err : "Unknown error", { cause: err });
	}
	function runErrors(err, fns, owner) {
		try {
			for (const f of fns) f(err);
		} catch (e) {
			handleError(e, owner && owner.owner || null);
		}
	}
	function handleError(err, owner = Owner) {
		const fns = ERROR && owner && owner.context && owner.context[ERROR];
		const error = castError(err);
		if (!fns) throw error;
		if (Effects) Effects.push({
			fn() {
				runErrors(error, fns, owner);
			},
			state: STALE
		});
		else runErrors(error, fns, owner);
	}
	var hydrationEnabled = false;
	function createComponent$1(Comp, props) {
		if (hydrationEnabled) {
			if (sharedConfig.context) {
				const c = sharedConfig.context;
				setHydrateContext(nextHydrateContext());
				const r = untrack(() => Comp(props || {}));
				setHydrateContext(c);
				return r;
			}
		}
		return untrack(() => Comp(props || {}));
	}
	function trueFn() {
		return true;
	}
	var propTraps = {
		get(_, property, receiver) {
			if (property === $PROXY) return receiver;
			return _.get(property);
		},
		has(_, property) {
			if (property === $PROXY) return true;
			return _.has(property);
		},
		set: trueFn,
		deleteProperty: trueFn,
		getOwnPropertyDescriptor(_, property) {
			return {
				configurable: true,
				enumerable: true,
				get() {
					return _.get(property);
				},
				set: trueFn,
				deleteProperty: trueFn
			};
		},
		ownKeys(_) {
			return _.keys();
		}
	};
	function resolveSource(s) {
		return !(s = typeof s === "function" ? s() : s) ? {} : s;
	}
	function resolveSources() {
		for (let i = 0, length = this.length; i < length; ++i) {
			const v = this[i]();
			if (v !== void 0) return v;
		}
	}
	function mergeProps$1(...sources) {
		let proxy = false;
		for (let i = 0; i < sources.length; i++) {
			const s = sources[i];
			proxy = proxy || !!s && $PROXY in s;
			sources[i] = typeof s === "function" ? (proxy = true, createMemo(s)) : s;
		}
		if (SUPPORTS_PROXY && proxy) return new Proxy({
			get(property) {
				for (let i = sources.length - 1; i >= 0; i--) {
					const v = resolveSource(sources[i])[property];
					if (v !== void 0) return v;
				}
			},
			has(property) {
				for (let i = sources.length - 1; i >= 0; i--) if (property in resolveSource(sources[i])) return true;
				return false;
			},
			keys() {
				const keys = [];
				for (let i = 0; i < sources.length; i++) keys.push(...Object.keys(resolveSource(sources[i])));
				return [...new Set(keys)];
			}
		}, propTraps);
		const sourcesMap = {};
		const defined = Object.create(null);
		for (let i = sources.length - 1; i >= 0; i--) {
			const source = sources[i];
			if (!source) continue;
			const sourceKeys = Object.getOwnPropertyNames(source);
			for (let i = sourceKeys.length - 1; i >= 0; i--) {
				const key = sourceKeys[i];
				if (key === "__proto__" || key === "constructor") continue;
				const desc = Object.getOwnPropertyDescriptor(source, key);
				if (!defined[key]) defined[key] = desc.get ? {
					enumerable: true,
					configurable: true,
					get: resolveSources.bind(sourcesMap[key] = [desc.get.bind(source)])
				} : desc.value !== void 0 ? desc : void 0;
				else {
					const sources = sourcesMap[key];
					if (sources) {
						if (desc.get) sources.push(desc.get.bind(source));
						else if (desc.value !== void 0) sources.push(() => desc.value);
					}
				}
			}
		}
		const target = {};
		const definedKeys = Object.keys(defined);
		for (let i = definedKeys.length - 1; i >= 0; i--) {
			const key = definedKeys[i], desc = defined[key];
			if (desc && desc.get) Object.defineProperty(target, key, desc);
			else target[key] = desc ? desc.value : void 0;
		}
		return target;
	}
	//#endregion
	//#region ../../../node_modules/.pnpm/solid-js@1.9.13/node_modules/solid-js/universal/dist/universal.js
	var memo$1 = (fn) => createMemo(() => fn());
	function createRenderer$1({ createElement, createTextNode, isTextNode, replaceText, insertNode, removeNode, setProperty, getParentNode, getFirstChild, getNextSibling }) {
		function insert(parent, accessor, marker, initial) {
			if (marker !== void 0 && !initial) initial = [];
			if (typeof accessor !== "function") return insertExpression(parent, accessor, initial, marker);
			createRenderEffect((current) => insertExpression(parent, accessor(), current, marker), initial);
		}
		function insertExpression(parent, value, current, marker, unwrapArray) {
			while (typeof current === "function") current = current();
			if (value === current) return current;
			const t = typeof value, multi = marker !== void 0;
			if (t === "string" || t === "number") {
				if (t === "number") value = value.toString();
				if (multi) {
					let node = current[0];
					if (node && isTextNode(node)) replaceText(node, value);
					else node = createTextNode(value);
					current = cleanChildren(parent, current, marker, node);
				} else if (current !== "" && typeof current === "string") replaceText(getFirstChild(parent), current = value);
				else {
					cleanChildren(parent, current, marker, createTextNode(value));
					current = value;
				}
			} else if (value == null || t === "boolean") current = cleanChildren(parent, current, marker);
			else if (t === "function") {
				createRenderEffect(() => {
					let v = value();
					while (typeof v === "function") v = v();
					current = insertExpression(parent, v, current, marker);
				});
				return () => current;
			} else if (Array.isArray(value)) {
				const array = [];
				if (normalizeIncomingArray(array, value, unwrapArray)) {
					createRenderEffect(() => current = insertExpression(parent, array, current, marker, true));
					return () => current;
				}
				if (array.length === 0) {
					const replacement = cleanChildren(parent, current, marker);
					if (multi) return current = replacement;
				} else if (Array.isArray(current)) if (current.length === 0) appendNodes(parent, array, marker);
				else reconcileArrays(parent, current, array);
				else if (current == null || current === "") appendNodes(parent, array);
				else reconcileArrays(parent, multi && current || [getFirstChild(parent)], array);
				current = array;
			} else {
				if (Array.isArray(current)) {
					if (multi) return current = cleanChildren(parent, current, marker, value);
					cleanChildren(parent, current, null, value);
				} else if (current == null || current === "" || !getFirstChild(parent)) insertNode(parent, value);
				else replaceNode(parent, value, getFirstChild(parent));
				current = value;
			}
			return current;
		}
		function normalizeIncomingArray(normalized, array, unwrap) {
			let dynamic = false;
			for (let i = 0, len = array.length; i < len; i++) {
				let item = array[i], t;
				if (item == null || item === true || item === false);
				else if (Array.isArray(item)) dynamic = normalizeIncomingArray(normalized, item) || dynamic;
				else if ((t = typeof item) === "string" || t === "number") normalized.push(createTextNode(item));
				else if (t === "function") if (unwrap) {
					while (typeof item === "function") item = item();
					dynamic = normalizeIncomingArray(normalized, Array.isArray(item) ? item : [item]) || dynamic;
				} else {
					normalized.push(item);
					dynamic = true;
				}
				else normalized.push(item);
			}
			return dynamic;
		}
		function reconcileArrays(parentNode, a, b) {
			let bLength = b.length, aEnd = a.length, bEnd = bLength, aStart = 0, bStart = 0, after = getNextSibling(a[aEnd - 1]), map = null;
			while (aStart < aEnd || bStart < bEnd) {
				if (a[aStart] === b[bStart]) {
					aStart++;
					bStart++;
					continue;
				}
				while (a[aEnd - 1] === b[bEnd - 1]) {
					aEnd--;
					bEnd--;
				}
				if (aEnd === aStart) {
					const node = bEnd < bLength ? bStart ? getNextSibling(b[bStart - 1]) : b[bEnd - bStart] : after;
					while (bStart < bEnd) insertNode(parentNode, b[bStart++], node);
				} else if (bEnd === bStart) while (aStart < aEnd) {
					if (!map || !map.has(a[aStart])) removeNode(parentNode, a[aStart]);
					aStart++;
				}
				else if (a[aStart] === b[bEnd - 1] && b[bStart] === a[aEnd - 1]) {
					const node = getNextSibling(a[--aEnd]);
					insertNode(parentNode, b[bStart++], getNextSibling(a[aStart++]));
					insertNode(parentNode, b[--bEnd], node);
					a[aEnd] = b[bEnd];
				} else {
					if (!map) {
						map = /* @__PURE__ */ new Map();
						let i = bStart;
						while (i < bEnd) map.set(b[i], i++);
					}
					const index = map.get(a[aStart]);
					if (index != null) if (bStart < index && index < bEnd) {
						let i = aStart, sequence = 1, t;
						while (++i < aEnd && i < bEnd) {
							if ((t = map.get(a[i])) == null || t !== index + sequence) break;
							sequence++;
						}
						if (sequence > index - bStart) {
							const node = a[aStart];
							while (bStart < index) insertNode(parentNode, b[bStart++], node);
						} else replaceNode(parentNode, b[bStart++], a[aStart++]);
					} else aStart++;
					else removeNode(parentNode, a[aStart++]);
				}
			}
		}
		function cleanChildren(parent, current, marker, replacement) {
			if (marker === void 0) {
				let removed;
				while (removed = getFirstChild(parent)) removeNode(parent, removed);
				replacement && insertNode(parent, replacement);
				return "";
			}
			const node = replacement || createTextNode("");
			if (current.length) {
				let inserted = false;
				for (let i = current.length - 1; i >= 0; i--) {
					const el = current[i];
					if (node !== el) {
						const isParent = getParentNode(el) === parent;
						if (!inserted && !i) isParent ? replaceNode(parent, node, el) : insertNode(parent, node, marker);
						else isParent && removeNode(parent, el);
					} else inserted = true;
				}
			} else insertNode(parent, node, marker);
			return [node];
		}
		function appendNodes(parent, array, marker) {
			for (let i = 0, len = array.length; i < len; i++) insertNode(parent, array[i], marker);
		}
		function replaceNode(parent, newNode, oldNode) {
			insertNode(parent, newNode, oldNode);
			removeNode(parent, oldNode);
		}
		function spreadExpression(node, props, prevProps = {}, skipChildren) {
			props || (props = {});
			if (!skipChildren) createRenderEffect(() => prevProps.children = insertExpression(node, props.children, prevProps.children));
			createRenderEffect(() => props.ref && props.ref(node));
			createRenderEffect(() => {
				for (const prop in props) {
					if (prop === "children" || prop === "ref") continue;
					const value = props[prop];
					if (value === prevProps[prop]) continue;
					setProperty(node, prop, value, prevProps[prop]);
					prevProps[prop] = value;
				}
			});
			return prevProps;
		}
		return {
			render(code, element) {
				let disposer;
				createRoot((dispose) => {
					disposer = dispose;
					insert(element, code());
				});
				return disposer;
			},
			insert,
			spread(node, accessor, skipChildren) {
				if (typeof accessor === "function") createRenderEffect((current) => spreadExpression(node, accessor(), current, skipChildren));
				else spreadExpression(node, accessor, void 0, skipChildren);
			},
			createElement,
			createTextNode,
			insertNode,
			setProp(node, name, value, prev) {
				setProperty(node, name, value, prev);
				return value;
			},
			mergeProps: mergeProps$1,
			effect: createRenderEffect,
			memo: memo$1,
			createComponent: createComponent$1,
			use(fn, element, arg) {
				return untrack(() => fn(element, arg));
			}
		};
	}
	function createRenderer(options) {
		const renderer = createRenderer$1(options);
		renderer.mergeProps = mergeProps$1;
		return renderer;
	}
	//#endregion
	//#region \0@oxc-project+runtime@0.132.0/helpers/typeof.js
	function _typeof(o) {
		"@babel/helpers - typeof";
		return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function(o) {
			return typeof o;
		} : function(o) {
			return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o;
		}, _typeof(o);
	}
	//#endregion
	//#region \0@oxc-project+runtime@0.132.0/helpers/toPrimitive.js
	function toPrimitive(t, r) {
		if ("object" != _typeof(t) || !t) return t;
		var e = t[Symbol.toPrimitive];
		if (void 0 !== e) {
			var i = e.call(t, r || "default");
			if ("object" != _typeof(i)) return i;
			throw new TypeError("@@toPrimitive must return a primitive value.");
		}
		return ("string" === r ? String : Number)(t);
	}
	//#endregion
	//#region \0@oxc-project+runtime@0.132.0/helpers/toPropertyKey.js
	function toPropertyKey(t) {
		var i = toPrimitive(t, "string");
		return "symbol" == _typeof(i) ? i : i + "";
	}
	//#endregion
	//#region \0@oxc-project+runtime@0.132.0/helpers/defineProperty.js
	function _defineProperty(e, r, t) {
		return (r = toPropertyKey(r)) in e ? Object.defineProperty(e, r, {
			value: t,
			enumerable: !0,
			configurable: !0,
			writable: !0
		}) : e[r] = t, e;
	}
	//#endregion
	//#region ../../packages/renderer-protocol/dist/index.js
	var asElementId = (n) => n;
	var ELEMENT_PROPERTY_NAMES = [
		"value",
		"placeholder",
		"src",
		"disabled",
		"user-select",
		"multiline"
	];
	function coerceElementProperty(name, value) {
		switch (name) {
			case "value": return {
				kind: "text-content",
				text: value == null ? "" : String(value)
			};
			case "placeholder": return {
				kind: "placeholder",
				text: typeof value === "string" ? value : ""
			};
			case "src": return {
				kind: "src",
				text: typeof value === "string" ? value : ""
			};
			case "disabled": return {
				kind: "disabled",
				disabled: Boolean(value)
			};
			case "user-select": return {
				kind: "user-select",
				value: value === "none" || value === "contains" ? value : "text"
			};
			case "multiline": return {
				kind: "multiline",
				multiline: Boolean(value)
			};
		}
	}
	function dispatchElementPropertyOp(op, effects) {
		const handler = effects[op.kind];
		return handler(op);
	}
	var KNOWN_PROPERTIES = new Set(ELEMENT_PROPERTY_NAMES);
	function isKnownElementProperty(name) {
		return KNOWN_PROPERTIES.has(name);
	}
	function assertKnownElementProperty(name) {
		if (!isKnownElementProperty(name)) throw new Error(`Unknown element property "${name}". Only ${ELEMENT_PROPERTY_NAMES.join(", ")} are allowed (ADR-0071).`);
	}
	var PSEUDO_STYLE_KEYS = [
		":focus",
		":hover",
		":active"
	];
	var PSEUDO_STATE_CODE = {
		":focus": 2,
		":hover": 0,
		":active": 1
	};
	function isPseudoStyleKey(key) {
		return PSEUDO_STYLE_KEYS.includes(key);
	}
	function splitHayateStyle(style) {
		const base = {};
		const pseudo = {};
		for (const [key, value] of Object.entries(style)) if (isPseudoStyleKey(key)) pseudo[key] = value ?? {};
		else base[key] = value;
		return {
			base,
			pseudo
		};
	}
	var TEXT_LOCAL_KEYS = /* @__PURE__ */ new Set([
		"fontSize",
		"color",
		"fontFamily",
		"fontWeight",
		"fontStyle",
		"textDecoration"
	]);
	function isTextLocal(key) {
		return TEXT_LOCAL_KEYS.has(key);
	}
	var TEXT_LOCAL_CARRIERS = /* @__PURE__ */ new Set(["text", "text-input"]);
	function carriesTextLocal(kind) {
		return TEXT_LOCAL_CARRIERS.has(kind);
	}
	function shouldApplyTextLocalPatch(kind, patchKey) {
		if (!isTextLocal(patchKey)) return true;
		return carriesTextLocal(kind);
	}
	function gateTextLocalPatch(kind, patch) {
		if (carriesTextLocal(kind)) return patch;
		const gated = {};
		for (const key in patch) {
			if (!shouldApplyTextLocalPatch(kind, key)) continue;
			gated[key] = patch[key];
		}
		return gated;
	}
	var GatingRenderer = class {
		constructor(inner) {
			_defineProperty(this, "inner", void 0);
			_defineProperty(this, "kinds", /* @__PURE__ */ new Map());
			this.inner = inner;
		}
		createElement(kind) {
			const id = this.inner.createElement(kind);
			this.kinds.set(id, kind);
			return id;
		}
		setRoot(id) {
			this.inner.setRoot(id);
		}
		appendChild(parent, child) {
			this.inner.appendChild(parent, child);
		}
		insertBefore(parent, child, before) {
			this.inner.insertBefore(parent, child, before);
		}
		removeChild(parent, child) {
			this.kinds.delete(child);
			this.inner.removeChild(parent, child);
		}
		setStyle(id, style) {
			this.inner.setStyle(id, this.gate(id, style));
		}
		setPseudoStyle(id, pseudo, style) {
			this.inner.setPseudoStyle(id, pseudo, this.gate(id, style));
		}
		setStyleVariant(id, condition, style) {
			this.inner.setStyleVariant(id, condition, this.gate(id, style));
		}
		setText(id, text) {
			this.inner.setText(id, text);
		}
		setProperty(id, name, value) {
			this.inner.setProperty(id, name, value);
		}
		addEventListener(id, event, handler) {
			return this.inner.addEventListener(id, event, handler);
		}
		resize(width, height) {
			this.inner.resize(width, height);
		}
		/**
		* 要素の kind が持たない text-local プロップを除去する。先行する
		* `createElement` がない id（kind 不明）はそのまま通す。
		*/
		gate(id, style) {
			const kind = this.kinds.get(id);
			return kind === void 0 ? style : gateTextLocalPatch(kind, style);
		}
	};
	function withTextLocalGate(inner) {
		return new GatingRenderer(inner);
	}
	//#endregion
	//#region ../../packages/solid/dist/index.js
	var active = null;
	function setActiveRenderer(renderer) {
		active = renderer;
	}
	function activeRenderer() {
		if (active === null) throw new Error("tsubame-solid: アクティブな Renderer が未設定です。renderTsubame() を使うか setActiveRenderer() を先に呼んでください。");
		return active;
	}
	function createElementNode(id, elementKind) {
		return {
			id,
			elementKind,
			parent: null,
			children: [],
			events: /* @__PURE__ */ new Map()
		};
	}
	var REJECTED_EVENT_PROPS = /* @__PURE__ */ new Set(["onHoverEnter", "onHoverLeave"]);
	var EVENT_PROP = {
		onClick: "click",
		onInput: "input",
		onKeyDown: "keydown",
		onFocus: "focus",
		onBlur: "blur"
	};
	function disposeEvents(node) {
		for (const unsub of node.events.values()) unsub();
		node.events.clear();
		for (const child of node.children) disposeEvents(child);
	}
	function insertIntoChildren(parent, node, anchor) {
		if (anchor != null) {
			const i = parent.children.indexOf(anchor);
			parent.children.splice(i < 0 ? parent.children.length : i, 0, node);
		} else parent.children.push(node);
	}
	var { render, effect, memo, createComponent, createElement, createTextNode, insertNode, insert, spread, setProp, mergeProps } = createRenderer({
		createElement(tag) {
			const kind = tag;
			return createElementNode(activeRenderer().createElement(kind), kind);
		},
		createTextNode(value) {
			const r = activeRenderer();
			const id = r.createElement("text");
			r.setText(id, value);
			return createElementNode(id, "text");
		},
		replaceText(textNode, value) {
			if (textNode.elementKind !== "text") return;
			activeRenderer().setText(textNode.id, value);
		},
		isTextNode(node) {
			return node.elementKind === "text";
		},
		setProperty(node, name, value) {
			const r = activeRenderer();
			if (name === "style") {
				const { base, pseudo } = splitHayateStyle(value ?? {});
				r.setStyle(node.id, base);
				for (const [key, block] of Object.entries(pseudo)) if (block !== void 0) r.setPseudoStyle(node.id, key, block);
				return;
			}
			if (name === "styleVariants") {
				const variants = value ?? [];
				for (const variant of variants) {
					const { base } = splitHayateStyle(variant.style ?? {});
					r.setStyleVariant(node.id, variant.condition, base);
				}
				return;
			}
			if (node.elementKind === "text") return;
			if (REJECTED_EVENT_PROPS.has(name)) throw new Error(`${name} is not supported in tsubame-solid. Use ':hover' in style for visual feedback (ADR-0056, ADR-0059).`);
			const eventKind = EVENT_PROP[name];
			if (eventKind !== void 0) {
				node.events.get(name)?.();
				node.events.delete(name);
				if (typeof value === "function") node.events.set(name, r.addEventListener(node.id, eventKind, value));
				return;
			}
			if (name === "children" || name === "ref") return;
			assertKnownElementProperty(name);
			r.setProperty(node.id, name, value);
		},
		insertNode(parent, node, anchor) {
			node.parent = parent;
			insertIntoChildren(parent, node, anchor);
			const r = activeRenderer();
			if (anchor == null) {
				r.appendChild(parent.id, node.id);
				return;
			}
			r.insertBefore(parent.id, node.id, anchor.id);
		},
		removeNode(parent, node) {
			const i = parent.children.indexOf(node);
			if (i >= 0) parent.children.splice(i, 1);
			node.parent = null;
			activeRenderer().removeChild(parent.id, node.id);
			disposeEvents(node);
		},
		getParentNode(node) {
			return node.parent ?? void 0;
		},
		getFirstChild(node) {
			return node.children[0];
		},
		getNextSibling(node) {
			const parent = node.parent;
			if (parent === null) return void 0;
			const i = parent.children.indexOf(node);
			return i >= 0 ? parent.children[i + 1] : void 0;
		}
	});
	function renderTsubame(code, target, options) {
		const renderer = withTextLocalGate(target);
		setActiveRenderer(renderer);
		const rootId = renderer.createElement("view");
		renderer.setRoot(rootId);
		const root = createElementNode(rootId, "view");
		let rafHandle = null;
		const notifyResize = (w, h) => {
			if (rafHandle !== null) cancelAnimationFrame(rafHandle);
			rafHandle = requestAnimationFrame(() => {
				rafHandle = null;
				renderer.resize(w, h);
			});
		};
		let cleanupResize = null;
		const el = options?.element;
		if (el !== void 0 && typeof ResizeObserver !== "undefined") {
			const ro = new ResizeObserver((entries) => {
				const entry = entries[0];
				if (!entry) return;
				const { width, height } = entry.contentRect;
				notifyResize(Math.round(width), Math.round(height));
			});
			ro.observe(el);
			cleanupResize = () => ro.disconnect();
		} else {
			const handler = () => notifyResize(window.innerWidth, window.innerHeight);
			window.addEventListener("resize", handler);
			cleanupResize = () => window.removeEventListener("resize", handler);
		}
		const dispose = render(code, root);
		return () => {
			if (rafHandle !== null) cancelAnimationFrame(rafHandle);
			cleanupResize?.();
			dispose();
		};
	}
	//#endregion
	//#region src/android-prelude.ts
	var g = globalThis;
	var nativeLog = g["__hayateLog"];
	if (g["console"] === void 0) {
		const make = (level) => (...args) => {
			nativeLog?.(level, args.map((a) => String(a)).join(" "));
		};
		g["console"] = {
			log: make("log"),
			info: make("info"),
			warn: make("warn"),
			error: make("error"),
			debug: make("debug")
		};
	}
	if (typeof g["queueMicrotask"] !== "function") g["queueMicrotask"] = (cb) => {
		Promise.resolve().then(cb);
	};
	if (typeof g["requestAnimationFrame"] !== "function") g["requestAnimationFrame"] = (_cb) => 0;
	if (typeof g["cancelAnimationFrame"] !== "function") g["cancelAnimationFrame"] = (_handle) => {};
	if (typeof g["fetch"] !== "function") g["fetch"] = () => Promise.reject(/* @__PURE__ */ new Error("fetch is unavailable on Android (ADR-0112)"));
	function createMemoryStorage() {
		const map = /* @__PURE__ */ new Map();
		return {
			get length() {
				return map.size;
			},
			clear: () => map.clear(),
			getItem: (key) => map.get(key) ?? null,
			key: (index) => [...map.keys()][index] ?? null,
			removeItem: (key) => {
				map.delete(key);
			},
			setItem: (key, value) => {
				map.set(key, String(value));
			}
		};
	}
	if (g["window"] === void 0) g["window"] = {
		addEventListener: (_type, _handler) => {},
		removeEventListener: (_type, _handler) => {},
		innerWidth: 0,
		innerHeight: 0,
		location: {
			search: "",
			href: "",
			pathname: "/"
		},
		localStorage: createMemoryStorage()
	};
	if (g["document"] === void 0) g["document"] = {
		documentElement: { style: {
			setProperty: (_name, _value) => {},
			getPropertyValue: (_name) => "",
			removeProperty: (_name) => ""
		} },
		body: {
			appendChild: (node) => node,
			removeChild: (node) => node
		},
		getElementById: (_id) => null,
		addEventListener: (_type, _handler) => {},
		removeEventListener: (_type, _handler) => {},
		baseURI: ""
	};
	//#endregion
	//#region ../../proto/generated/protocol.ts
	var OP = {
		APPEND_CHILD: 0,
		INSERT_BEFORE: 1,
		REMOVE: 2,
		SET_ROOT: 3,
		SET_STYLE: 4,
		SET_TRANSFORM: 5,
		SET_SCROLL_OFFSET: 6,
		FOCUS: 7,
		BLUR: 8,
		CREATE: 9,
		SET_TEXT: 10,
		UNSET_STYLE: 11,
		SET_TEXT_CONTENT: 12,
		SET_DISABLED: 13,
		SET_SRC: 14,
		SET_PSEUDO_STYLE: 15,
		SET_STYLE_VARIANT: 16,
		SET_USER_SELECT: 17,
		SET_MULTILINE: 18
	};
	var TAG = {
		BACKGROUND_COLOR: 0,
		OPACITY: 1,
		BORDER_RADIUS: 2,
		BORDER_WIDTH: 3,
		BORDER_COLOR: 4,
		WIDTH: 5,
		HEIGHT: 6,
		MIN_WIDTH: 7,
		MIN_HEIGHT: 8,
		MAX_WIDTH: 9,
		MAX_HEIGHT: 10,
		DISPLAY: 11,
		FLEX_DIRECTION: 12,
		ALIGN_ITEMS: 13,
		JUSTIFY_CONTENT: 14,
		GAP: 15,
		PADDING: 16,
		PADDING_TOP: 17,
		PADDING_RIGHT: 18,
		PADDING_BOTTOM: 19,
		PADDING_LEFT: 20,
		MARGIN: 21,
		MARGIN_TOP: 22,
		MARGIN_RIGHT: 23,
		MARGIN_BOTTOM: 24,
		MARGIN_LEFT: 25,
		FONT_SIZE: 26,
		COLOR: 27,
		Z_INDEX: 28,
		FONT_FAMILY: 29,
		FLEX_GROW: 30,
		FONT_WEIGHT: 31,
		FONT_STYLE: 32,
		TEXT_DECORATION: 33,
		DEFAULT_COLOR: 34,
		DEFAULT_FONT_FAMILY: 35,
		DEFAULT_FONT_SIZE: 36,
		DEFAULT_FONT_WEIGHT: 37,
		GRID_TEMPLATE_COLUMNS: 38,
		GRID_TEMPLATE_ROWS: 39,
		FLEX_SHRINK: 40,
		FLEX_BASIS: 41,
		ALIGN_SELF: 42,
		ALIGN_CONTENT: 43,
		FLEX_WRAP: 44,
		BORDER_STYLE: 45,
		CURSOR: 46,
		POSITION: 47,
		TOP: 48,
		LEFT: 49,
		RIGHT: 50,
		BOTTOM: 51,
		OVERFLOW: 52,
		MAX_LINES: 53,
		TEXT_OVERFLOW: 54,
		TRANSITION_DURATION: 55,
		TRANSITION_TIMING: 56,
		BOX_SHADOW: 57
	};
	var EVENT_KIND = {
		CLICK: 0,
		FOCUS: 1,
		BLUR: 2,
		TEXT_INPUT: 3,
		COMPOSITION_START: 4,
		COMPOSITION_UPDATE: 5,
		COMPOSITION_END: 6,
		SCROLL: 7,
		RESIZE: 8,
		ACTIVE_END: 9,
		HOVER_ENTER: 10,
		HOVER_LEAVE: 11,
		KEY_DOWN: 12,
		ACTIVE_START: 13,
		POINTER_MOVE: 14,
		FETCH_FONT: 15,
		SELECTION_CHANGE: 16
	};
	var ELEMENT_KIND = {
		"view": 0,
		"text": 1,
		"image": 2,
		"button": 3,
		"text-input": 4,
		"scroll-view": 5
	};
	var UNSET_KIND = {
		color: 0,
		fontSize: 1,
		fontFamily: 2,
		fontWeight: 3
	};
	var DIMENSION_UNIT = {
		px: 0,
		percent: 1,
		auto: 2,
		fr: 3
	};
	var DISPLAY = {
		flex: 0,
		grid: 1,
		block: 2,
		none: 3
	};
	var FLEX_DIRECTION = {
		row: 0,
		column: 1,
		rowReverse: 2,
		columnReverse: 3
	};
	var ALIGN_ITEMS = {
		flexStart: 0,
		flexEnd: 1,
		center: 2,
		stretch: 3,
		baseline: 4
	};
	var JUSTIFY_CONTENT = {
		flexStart: 0,
		flexEnd: 1,
		center: 2,
		spaceBetween: 3,
		spaceAround: 4,
		spaceEvenly: 5
	};
	var FONT_STYLE = {
		normal: 0,
		italic: 1,
		oblique: 2
	};
	var ALIGN_SELF = {
		auto: 0,
		flexStart: 1,
		flexEnd: 2,
		center: 3,
		stretch: 4,
		baseline: 5
	};
	var FLEX_WRAP = {
		nowrap: 0,
		wrap: 1,
		wrapReverse: 2
	};
	var ALIGN_CONTENT = {
		flexStart: 0,
		flexEnd: 1,
		center: 2,
		stretch: 3,
		spaceBetween: 4,
		spaceAround: 5,
		spaceEvenly: 6
	};
	var TEXT_DECORATION = {
		none: 0,
		underline: 1,
		lineThrough: 2
	};
	var BORDER_STYLE = {
		none: 0,
		solid: 1,
		dashed: 2
	};
	var OVERFLOW = {
		visible: 0,
		hidden: 1
	};
	var TEXT_OVERFLOW = {
		clip: 0,
		ellipsis: 1
	};
	var CURSOR = {
		default: 0,
		pointer: 1,
		text: 2,
		crosshair: 3,
		notAllowed: 4,
		grab: 5,
		grabbing: 6
	};
	var POSITION = {
		relative: 0,
		absolute: 1
	};
	var TRANSITION_TIMING = {
		ease: 0,
		linear: 1,
		easeIn: 2,
		easeOut: 3,
		easeInOut: 4
	};
	var USER_SELECT = {
		text: 0,
		none: 1,
		contains: 2
	};
	var UNIT_CODE = DIMENSION_UNIT;
	function parseEvent(ev) {
		const kind = ev[0];
		switch (kind) {
			case 0: return {
				kind: "click",
				value: 0,
				targetId: ev[1],
				x: ev[2],
				y: ev[3]
			};
			case 1: return {
				kind: "focus",
				value: 1,
				targetId: ev[1]
			};
			case 2: return {
				kind: "blur",
				value: 2,
				targetId: ev[1]
			};
			case 3: return {
				kind: "text_input",
				value: 3,
				targetId: ev[1],
				text: ev[2]
			};
			case 4: return {
				kind: "composition_start",
				value: 4,
				targetId: ev[1],
				text: ev[2]
			};
			case 5: return {
				kind: "composition_update",
				value: 5,
				targetId: ev[1],
				text: ev[2]
			};
			case 6: return {
				kind: "composition_end",
				value: 6,
				targetId: ev[1],
				text: ev[2]
			};
			case 7: return {
				kind: "scroll",
				value: 7,
				targetId: ev[1],
				deltaX: ev[2],
				deltaY: ev[3]
			};
			case 8: return {
				kind: "resize",
				value: 8,
				width: ev[1],
				height: ev[2]
			};
			case 9: return {
				kind: "active_end",
				value: 9,
				targetId: ev[1]
			};
			case 10: return {
				kind: "hover_enter",
				value: 10,
				targetId: ev[1]
			};
			case 11: return {
				kind: "hover_leave",
				value: 11,
				targetId: ev[1]
			};
			case 12: return {
				kind: "key_down",
				value: 12,
				targetId: ev[1],
				key: ev[2],
				modifiers: ev[3]
			};
			case 13: return {
				kind: "active_start",
				value: 13,
				targetId: ev[1]
			};
			case 14: return {
				kind: "pointer_move",
				value: 14,
				x: ev[1],
				y: ev[2],
				pointerKind: ev[3]
			};
			case 15: return {
				kind: "fetch_font",
				value: 15,
				family: ev[1]
			};
			case 16: return {
				kind: "selection_change",
				value: 16
			};
			default: throw new Error(`parseEvent: unknown event kind ${kind}`);
		}
	}
	//#endregion
	//#region ../../proto/generated/codec.ts
	function finiteNumber(key, value) {
		const numeric = Number(value);
		if (!Number.isFinite(numeric)) throw new Error(`CanvasRenderer: invalid numeric value for "${key}"`);
		return numeric;
	}
	function finiteInteger(key, value) {
		const numeric = finiteNumber(key, value);
		if (!Number.isInteger(numeric)) throw new Error(`CanvasRenderer: "${key}" must be an integer`);
		return numeric;
	}
	function parseDimension(value) {
		if (typeof value === "number") return {
			value,
			unit: "px"
		};
		const trimmed = value.trim().toLowerCase();
		if (trimmed === "auto") return {
			value: 0,
			unit: "auto"
		};
		const match = trimmed.match(/^(-?(?:\d+|\d*\.\d+))(px|%|fr)?$/);
		if (match === null) throw new Error(`CanvasRenderer: unsupported dimension "${value}"`);
		const numeric = Number(match[1]);
		if (!Number.isFinite(numeric)) throw new Error(`CanvasRenderer: invalid dimension "${value}"`);
		const unit = match[2] ?? "px";
		if (unit === "%") return {
			value: numeric,
			unit: "percent"
		};
		if (unit === "fr") return {
			value: numeric,
			unit: "fr"
		};
		return {
			value: numeric,
			unit: "px"
		};
	}
	function parseColor(input) {
		const s = input.trim().toLowerCase();
		if (s.startsWith("#")) {
			const hex = s.slice(1);
			const read1 = (i) => parseInt(hex[i] + hex[i], 16) / 255;
			const read2 = (i) => parseInt(hex.slice(i, i + 2), 16) / 255;
			if (hex.length === 3) return {
				r: read1(0),
				g: read1(1),
				b: read1(2),
				a: 1
			};
			if (hex.length === 4) return {
				r: read1(0),
				g: read1(1),
				b: read1(2),
				a: read1(3)
			};
			if (hex.length === 6) return {
				r: read2(0),
				g: read2(2),
				b: read2(4),
				a: 1
			};
			if (hex.length === 8) return {
				r: read2(0),
				g: read2(2),
				b: read2(4),
				a: read2(6)
			};
		}
		const rgb = s.match(/^rgba?\((.*)\)$/);
		if (rgb !== null) {
			const parts = rgb[1].replace(/\s*\/\s*/, ",").replace(/\s+/g, ",").split(",").filter(Boolean);
			if (parts.length >= 3) return {
				r: parseColorChannel(parts[0]),
				g: parseColorChannel(parts[1]),
				b: parseColorChannel(parts[2]),
				a: parts[3] === void 0 ? 1 : parseAlpha(parts[3])
			};
		}
		if (s === "transparent") return {
			r: 0,
			g: 0,
			b: 0,
			a: 0
		};
		throw new Error(`CanvasRenderer: unsupported color "${input}"`);
	}
	function parseColorChannel(raw) {
		const value = raw.trim();
		if (value.endsWith("%")) return clamp01(parseFloat(value) / 100);
		return clamp01(parseFloat(value) / 255);
	}
	function parseAlpha(raw) {
		const value = raw.trim();
		if (value.endsWith("%")) return clamp01(parseFloat(value) / 100);
		return clamp01(parseFloat(value));
	}
	function clamp01(value) {
		if (!Number.isFinite(value)) return 0;
		return Math.min(1, Math.max(0, value));
	}
	var DISPLAY_CODE = {
		"flex": DISPLAY.flex,
		"grid": DISPLAY.grid,
		"block": DISPLAY.block,
		"none": DISPLAY.none
	};
	var FLEX_DIRECTION_CODE = {
		"row": FLEX_DIRECTION.row,
		"column": FLEX_DIRECTION.column,
		"row-reverse": FLEX_DIRECTION.rowReverse,
		"column-reverse": FLEX_DIRECTION.columnReverse
	};
	var FLEX_WRAP_CODE = {
		"nowrap": FLEX_WRAP.nowrap,
		"wrap": FLEX_WRAP.wrap,
		"wrap-reverse": FLEX_WRAP.wrapReverse
	};
	var ALIGN_ITEMS_CODE = {
		"flex-start": ALIGN_ITEMS.flexStart,
		"flex-end": ALIGN_ITEMS.flexEnd,
		"center": ALIGN_ITEMS.center,
		"stretch": ALIGN_ITEMS.stretch,
		"baseline": ALIGN_ITEMS.baseline
	};
	var ALIGN_SELF_CODE = {
		"auto": ALIGN_SELF.auto,
		"flex-start": ALIGN_SELF.flexStart,
		"flex-end": ALIGN_SELF.flexEnd,
		"center": ALIGN_SELF.center,
		"stretch": ALIGN_SELF.stretch,
		"baseline": ALIGN_SELF.baseline
	};
	var ALIGN_CONTENT_CODE = {
		"flex-start": ALIGN_CONTENT.flexStart,
		"flex-end": ALIGN_CONTENT.flexEnd,
		"center": ALIGN_CONTENT.center,
		"stretch": ALIGN_CONTENT.stretch,
		"space-between": ALIGN_CONTENT.spaceBetween,
		"space-around": ALIGN_CONTENT.spaceAround,
		"space-evenly": ALIGN_CONTENT.spaceEvenly
	};
	var JUSTIFY_CONTENT_CODE = {
		"flex-start": JUSTIFY_CONTENT.flexStart,
		"flex-end": JUSTIFY_CONTENT.flexEnd,
		"center": JUSTIFY_CONTENT.center,
		"space-between": JUSTIFY_CONTENT.spaceBetween,
		"space-around": JUSTIFY_CONTENT.spaceAround,
		"space-evenly": JUSTIFY_CONTENT.spaceEvenly
	};
	var FONT_STYLE_CODE = {
		"normal": FONT_STYLE.normal,
		"italic": FONT_STYLE.italic,
		"oblique": FONT_STYLE.oblique
	};
	var TEXT_DECORATION_CODE = {
		"none": TEXT_DECORATION.none,
		"underline": TEXT_DECORATION.underline,
		"line-through": TEXT_DECORATION.lineThrough
	};
	var BORDER_STYLE_CODE = {
		"none": BORDER_STYLE.none,
		"solid": BORDER_STYLE.solid,
		"dashed": BORDER_STYLE.dashed
	};
	var CURSOR_CODE = {
		"default": CURSOR.default,
		"pointer": CURSOR.pointer,
		"text": CURSOR.text,
		"crosshair": CURSOR.crosshair,
		"not-allowed": CURSOR.notAllowed,
		"grab": CURSOR.grab,
		"grabbing": CURSOR.grabbing
	};
	var OVERFLOW_CODE = {
		"visible": OVERFLOW.visible,
		"hidden": OVERFLOW.hidden
	};
	var TEXT_OVERFLOW_CODE = {
		"clip": TEXT_OVERFLOW.clip,
		"ellipsis": TEXT_OVERFLOW.ellipsis
	};
	var POSITION_CODE = {
		"relative": POSITION.relative,
		"absolute": POSITION.absolute
	};
	var TRANSITION_TIMING_CODE = {
		"ease": TRANSITION_TIMING.ease,
		"linear": TRANSITION_TIMING.linear,
		"ease-in": TRANSITION_TIMING.easeIn,
		"ease-out": TRANSITION_TIMING.easeOut,
		"ease-in-out": TRANSITION_TIMING.easeInOut
	};
	function encode_backgroundColor(out, value) {
		const c = parseColor(value);
		out.push(TAG.BACKGROUND_COLOR, c.r, c.g, c.b, c.a);
	}
	function encode_opacity(out, value) {
		out.push(TAG.OPACITY, finiteNumber("opacity", value));
	}
	function encode_borderRadius(out, value) {
		out.push(TAG.BORDER_RADIUS, finiteNumber("borderRadius", value));
	}
	function encode_borderWidth(out, value) {
		out.push(TAG.BORDER_WIDTH, finiteNumber("borderWidth", value));
	}
	function encode_borderColor(out, value) {
		const c = parseColor(value);
		out.push(TAG.BORDER_COLOR, c.r, c.g, c.b, c.a);
	}
	function encode_width(out, value) {
		const d = parseDimension(value);
		out.push(TAG.WIDTH, d.value, UNIT_CODE[d.unit]);
	}
	function encode_height(out, value) {
		const d = parseDimension(value);
		out.push(TAG.HEIGHT, d.value, UNIT_CODE[d.unit]);
	}
	function encode_minWidth(out, value) {
		const d = parseDimension(value);
		out.push(TAG.MIN_WIDTH, d.value, UNIT_CODE[d.unit]);
	}
	function encode_minHeight(out, value) {
		const d = parseDimension(value);
		out.push(TAG.MIN_HEIGHT, d.value, UNIT_CODE[d.unit]);
	}
	function encode_maxWidth(out, value) {
		const d = parseDimension(value);
		out.push(TAG.MAX_WIDTH, d.value, UNIT_CODE[d.unit]);
	}
	function encode_maxHeight(out, value) {
		const d = parseDimension(value);
		out.push(TAG.MAX_HEIGHT, d.value, UNIT_CODE[d.unit]);
	}
	function encode_display(out, value) {
		const code = DISPLAY_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported display "${value}"`);
		out.push(TAG.DISPLAY, code);
	}
	function encode_flexDirection(out, value) {
		const code = FLEX_DIRECTION_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported flexDirection "${value}"`);
		out.push(TAG.FLEX_DIRECTION, code);
	}
	function encode_alignItems(out, value) {
		const code = ALIGN_ITEMS_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported alignItems "${value}"`);
		out.push(TAG.ALIGN_ITEMS, code);
	}
	function encode_justifyContent(out, value) {
		const code = JUSTIFY_CONTENT_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported justifyContent "${value}"`);
		out.push(TAG.JUSTIFY_CONTENT, code);
	}
	function encode_gap(out, value) {
		const d = parseDimension(value);
		out.push(TAG.GAP, d.value, UNIT_CODE[d.unit]);
	}
	function encode_padding(out, value) {
		const d = parseDimension(value);
		out.push(TAG.PADDING, d.value, UNIT_CODE[d.unit]);
	}
	function encode_paddingTop(out, value) {
		const d = parseDimension(value);
		out.push(TAG.PADDING_TOP, d.value, UNIT_CODE[d.unit]);
	}
	function encode_paddingRight(out, value) {
		const d = parseDimension(value);
		out.push(TAG.PADDING_RIGHT, d.value, UNIT_CODE[d.unit]);
	}
	function encode_paddingBottom(out, value) {
		const d = parseDimension(value);
		out.push(TAG.PADDING_BOTTOM, d.value, UNIT_CODE[d.unit]);
	}
	function encode_paddingLeft(out, value) {
		const d = parseDimension(value);
		out.push(TAG.PADDING_LEFT, d.value, UNIT_CODE[d.unit]);
	}
	function encode_margin(out, value) {
		const d = parseDimension(value);
		out.push(TAG.MARGIN, d.value, UNIT_CODE[d.unit]);
	}
	function encode_marginTop(out, value) {
		const d = parseDimension(value);
		out.push(TAG.MARGIN_TOP, d.value, UNIT_CODE[d.unit]);
	}
	function encode_marginRight(out, value) {
		const d = parseDimension(value);
		out.push(TAG.MARGIN_RIGHT, d.value, UNIT_CODE[d.unit]);
	}
	function encode_marginBottom(out, value) {
		const d = parseDimension(value);
		out.push(TAG.MARGIN_BOTTOM, d.value, UNIT_CODE[d.unit]);
	}
	function encode_marginLeft(out, value) {
		const d = parseDimension(value);
		out.push(TAG.MARGIN_LEFT, d.value, UNIT_CODE[d.unit]);
	}
	function encode_fontSize(out, value) {
		out.push(TAG.FONT_SIZE, finiteNumber("fontSize", value));
	}
	function encode_color(out, value) {
		const c = parseColor(value);
		out.push(TAG.COLOR, c.r, c.g, c.b, c.a);
	}
	function encode_zIndex(out, value) {
		out.push(TAG.Z_INDEX, finiteInteger("zIndex", value));
	}
	function encode_fontFamily(out, value) {
		const bytes = new TextEncoder().encode(value);
		out.push(TAG.FONT_FAMILY, bytes.length);
		for (const byte of bytes) out.push(byte);
	}
	function encode_flexGrow(out, value) {
		out.push(TAG.FLEX_GROW, finiteNumber("flexGrow", value));
	}
	function encode_fontWeight(out, value) {
		out.push(TAG.FONT_WEIGHT, finiteNumber("fontWeight", value));
	}
	function encode_fontStyle(out, value) {
		const code = FONT_STYLE_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported fontStyle "${value}"`);
		out.push(TAG.FONT_STYLE, code);
	}
	function encode_textDecoration(out, value) {
		const code = TEXT_DECORATION_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported textDecoration "${value}"`);
		out.push(TAG.TEXT_DECORATION, code);
	}
	function encode_defaultColor(out, value) {
		const c = parseColor(value);
		out.push(TAG.DEFAULT_COLOR, c.r, c.g, c.b, c.a);
	}
	function encode_defaultFontFamily(out, value) {
		const bytes = new TextEncoder().encode(value);
		out.push(TAG.DEFAULT_FONT_FAMILY, bytes.length);
		for (const byte of bytes) out.push(byte);
	}
	function encode_defaultFontSize(out, value) {
		out.push(TAG.DEFAULT_FONT_SIZE, finiteNumber("defaultFontSize", value));
	}
	function encode_defaultFontWeight(out, value) {
		out.push(TAG.DEFAULT_FONT_WEIGHT, finiteNumber("defaultFontWeight", value));
	}
	function encode_gridTemplateColumns(out, value) {
		if (!Array.isArray(value)) throw new Error(`CanvasRenderer: "gridTemplateColumns" must be an array of dimensions`);
		out.push(TAG.GRID_TEMPLATE_COLUMNS, value.length);
		for (const item of value) {
			const d = parseDimension(item);
			out.push(d.value, UNIT_CODE[d.unit]);
		}
	}
	function encode_gridTemplateRows(out, value) {
		if (!Array.isArray(value)) throw new Error(`CanvasRenderer: "gridTemplateRows" must be an array of dimensions`);
		out.push(TAG.GRID_TEMPLATE_ROWS, value.length);
		for (const item of value) {
			const d = parseDimension(item);
			out.push(d.value, UNIT_CODE[d.unit]);
		}
	}
	function encode_flexShrink(out, value) {
		out.push(TAG.FLEX_SHRINK, finiteNumber("flexShrink", value));
	}
	function encode_flexBasis(out, value) {
		const d = parseDimension(value);
		out.push(TAG.FLEX_BASIS, d.value, UNIT_CODE[d.unit]);
	}
	function encode_alignSelf(out, value) {
		const code = ALIGN_SELF_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported alignSelf "${value}"`);
		out.push(TAG.ALIGN_SELF, code);
	}
	function encode_alignContent(out, value) {
		const code = ALIGN_CONTENT_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported alignContent "${value}"`);
		out.push(TAG.ALIGN_CONTENT, code);
	}
	function encode_flexWrap(out, value) {
		const code = FLEX_WRAP_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported flexWrap "${value}"`);
		out.push(TAG.FLEX_WRAP, code);
	}
	function encode_borderStyle(out, value) {
		const code = BORDER_STYLE_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported borderStyle "${value}"`);
		out.push(TAG.BORDER_STYLE, code);
	}
	function encode_cursor(out, value) {
		const code = CURSOR_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported cursor "${value}"`);
		out.push(TAG.CURSOR, code);
	}
	function encode_position(out, value) {
		const code = POSITION_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported position "${value}"`);
		out.push(TAG.POSITION, code);
	}
	function encode_top(out, value) {
		const d = parseDimension(value);
		out.push(TAG.TOP, d.value, UNIT_CODE[d.unit]);
	}
	function encode_left(out, value) {
		const d = parseDimension(value);
		out.push(TAG.LEFT, d.value, UNIT_CODE[d.unit]);
	}
	function encode_right(out, value) {
		const d = parseDimension(value);
		out.push(TAG.RIGHT, d.value, UNIT_CODE[d.unit]);
	}
	function encode_bottom(out, value) {
		const d = parseDimension(value);
		out.push(TAG.BOTTOM, d.value, UNIT_CODE[d.unit]);
	}
	function encode_overflow(out, value) {
		const code = OVERFLOW_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported overflow "${value}"`);
		out.push(TAG.OVERFLOW, code);
	}
	function encode_maxLines(out, value) {
		out.push(TAG.MAX_LINES, finiteInteger("maxLines", value));
	}
	function encode_textOverflow(out, value) {
		const code = TEXT_OVERFLOW_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported textOverflow "${value}"`);
		out.push(TAG.TEXT_OVERFLOW, code);
	}
	function encode_transitionDuration(out, value) {
		out.push(TAG.TRANSITION_DURATION, finiteNumber("transitionDuration", value));
	}
	function encode_transitionTiming(out, value) {
		const code = TRANSITION_TIMING_CODE[value];
		if (code === void 0) throw new Error(`CanvasRenderer: unsupported transitionTiming "${value}"`);
		out.push(TAG.TRANSITION_TIMING, code);
	}
	function encode_boxShadow(out, value) {
		if (!Array.isArray(value)) throw new Error(`CanvasRenderer: "boxShadow" must be an array of shadows`);
		out.push(TAG.BOX_SHADOW, value.length);
		for (const item of value) {
			const c = parseColor(item.color);
			out.push(finiteNumber("boxShadow.offsetX", item.offsetX), finiteNumber("boxShadow.offsetY", item.offsetY), finiteNumber("boxShadow.blur", item.blur), finiteNumber("boxShadow.spread", item.spread), c.r, c.g, c.b, c.a, item.inset ? 1 : 0);
		}
	}
	var STYLE_ENCODERS = {
		backgroundColor: encode_backgroundColor,
		opacity: encode_opacity,
		borderRadius: encode_borderRadius,
		borderWidth: encode_borderWidth,
		borderColor: encode_borderColor,
		width: encode_width,
		height: encode_height,
		minWidth: encode_minWidth,
		minHeight: encode_minHeight,
		maxWidth: encode_maxWidth,
		maxHeight: encode_maxHeight,
		display: encode_display,
		flexDirection: encode_flexDirection,
		alignItems: encode_alignItems,
		justifyContent: encode_justifyContent,
		gap: encode_gap,
		padding: encode_padding,
		paddingTop: encode_paddingTop,
		paddingRight: encode_paddingRight,
		paddingBottom: encode_paddingBottom,
		paddingLeft: encode_paddingLeft,
		margin: encode_margin,
		marginTop: encode_marginTop,
		marginRight: encode_marginRight,
		marginBottom: encode_marginBottom,
		marginLeft: encode_marginLeft,
		fontSize: encode_fontSize,
		color: encode_color,
		zIndex: encode_zIndex,
		fontFamily: encode_fontFamily,
		flexGrow: encode_flexGrow,
		fontWeight: encode_fontWeight,
		fontStyle: encode_fontStyle,
		textDecoration: encode_textDecoration,
		defaultColor: encode_defaultColor,
		defaultFontFamily: encode_defaultFontFamily,
		defaultFontSize: encode_defaultFontSize,
		defaultFontWeight: encode_defaultFontWeight,
		gridTemplateColumns: encode_gridTemplateColumns,
		gridTemplateRows: encode_gridTemplateRows,
		flexShrink: encode_flexShrink,
		flexBasis: encode_flexBasis,
		alignSelf: encode_alignSelf,
		alignContent: encode_alignContent,
		flexWrap: encode_flexWrap,
		borderStyle: encode_borderStyle,
		cursor: encode_cursor,
		position: encode_position,
		top: encode_top,
		left: encode_left,
		right: encode_right,
		bottom: encode_bottom,
		overflow: encode_overflow,
		maxLines: encode_maxLines,
		textOverflow: encode_textOverflow,
		transitionDuration: encode_transitionDuration,
		transitionTiming: encode_transitionTiming,
		boxShadow: encode_boxShadow
	};
	var INHERITED_UNSET = {
		color: UNSET_KIND.color,
		fontSize: UNSET_KIND.fontSize,
		fontFamily: UNSET_KIND.fontFamily,
		fontWeight: UNSET_KIND.fontWeight
	};
	/** StylePatch の SET 部分を style-packet の TAG ワイヤースロットへエンコードする。 */
	function encodeStylePatch(patch, out) {
		for (const key in patch) {
			const k = key;
			const value = patch[k];
			if (value === void 0 || value === null) continue;
			const encoder = STYLE_ENCODERS[k];
			if (encoder === void 0) throw new Error(`CanvasRenderer: unsupported style property "${String(k)}"`);
			encoder(out, value);
		}
	}
	/** StylePatch 内の継承プロパティの null リセットを OP_UNSET_STYLE の種別コードへ対応付ける。 */
	function unsetKindsOf(patch) {
		const kinds = [];
		for (const key in patch) {
			const k = key;
			if (patch[k] !== null) continue;
			const code = INHERITED_UNSET[k];
			if (code === void 0) throw new Error(`CanvasRenderer: cannot reset non-inheritable property "${String(k)}"`);
			kinds.push(code);
		}
		return kinds;
	}
	function appendChild(buf, parentId, childId) {
		buf.push(OP.APPEND_CHILD);
		buf.push(parentId);
		buf.push(childId);
	}
	function insertBefore(buf, parentId, childId, beforeId) {
		buf.push(OP.INSERT_BEFORE);
		buf.push(parentId);
		buf.push(childId);
		buf.push(beforeId);
	}
	function appendRemove(buf, id) {
		buf.push(OP.REMOVE);
		buf.push(id);
	}
	function appendSetRoot(buf, id) {
		buf.push(OP.SET_ROOT);
		buf.push(id);
	}
	function appendSetStyle(buf, id, styleOffset, styleLen) {
		buf.push(OP.SET_STYLE);
		buf.push(id);
		buf.push(styleOffset);
		buf.push(styleLen);
	}
	function appendCreate(buf, id, kind) {
		buf.push(OP.CREATE);
		buf.push(id);
		buf.push(kind);
	}
	function appendSetText(buf, id, textIndex) {
		buf.push(OP.SET_TEXT);
		buf.push(id);
		buf.push(textIndex);
	}
	function appendUnsetStyle(buf, id, kind) {
		buf.push(OP.UNSET_STYLE);
		buf.push(id);
		buf.push(kind);
	}
	function appendSetTextContent(buf, id, textIndex) {
		buf.push(OP.SET_TEXT_CONTENT);
		buf.push(id);
		buf.push(textIndex);
	}
	function appendSetDisabled(buf, id, disabled) {
		buf.push(OP.SET_DISABLED);
		buf.push(id);
		buf.push(disabled);
	}
	function appendSetSrc(buf, id, textIndex) {
		buf.push(OP.SET_SRC);
		buf.push(id);
		buf.push(textIndex);
	}
	function appendSetPseudoStyle(buf, id, state, styleOffset, styleLen) {
		buf.push(OP.SET_PSEUDO_STYLE);
		buf.push(id);
		buf.push(state);
		buf.push(styleOffset);
		buf.push(styleLen);
	}
	function appendSetStyleVariant(buf, id, minWidth, maxWidth, minHeight, maxHeight, styleOffset, styleLen) {
		buf.push(OP.SET_STYLE_VARIANT);
		buf.push(id);
		buf.push(minWidth);
		buf.push(maxWidth);
		buf.push(minHeight);
		buf.push(maxHeight);
		buf.push(styleOffset);
		buf.push(styleLen);
	}
	function appendSetUserSelect(buf, id, value) {
		buf.push(OP.SET_USER_SELECT);
		buf.push(id);
		buf.push(value);
	}
	function appendSetMultiline(buf, id, multiline) {
		buf.push(OP.SET_MULTILINE);
		buf.push(id);
		buf.push(multiline);
	}
	//#endregion
	//#region ../../proto/generated/delivery.ts
	/** Hayate の `register_listener` で登録可能な EventKind（adapterTier: forward）。 */
	var HAYATE_LISTENER_KIND = {
		"click": EVENT_KIND.CLICK,
		"focus": EVENT_KIND.FOCUS,
		"blur": EVENT_KIND.BLUR,
		"input": EVENT_KIND.TEXT_INPUT,
		"hover-enter": EVENT_KIND.HOVER_ENTER,
		"hover-leave": EVENT_KIND.HOVER_LEAVE,
		"keydown": EVENT_KIND.KEY_DOWN
	};
	EVENT_KIND.COMPOSITION_START, EVENT_KIND.COMPOSITION_UPDATE, EVENT_KIND.COMPOSITION_END, EVENT_KIND.SCROLL;
	var IGNORED_KINDS = new Set([
		"composition_start",
		"composition_update",
		"composition_end",
		"scroll",
		"resize",
		"active_end",
		"active_start",
		"pointer_move",
		"fetch_font",
		"selection_change"
	]);
	/** Hayate の `poll_events()` の配信行 `[listener_id, kind, ...fields]` を1件デコードする。 */
	function parseDelivery(row) {
		return {
			listenerId: row[0],
			event: parseEvent(row.slice(1))
		};
	}
	/** 解析済みの Hayate イベントペイロードを、配信可能なら Tsubame の {@link InteractionEvent} へ変換する。 */
	function toInteractionEvent(ev) {
		if (IGNORED_KINDS.has(ev.kind)) return null;
		switch (ev.kind) {
			case "click": return {
				kind: "click",
				target: asElementId(ev.targetId)
			};
			case "focus": return {
				kind: "focus",
				target: asElementId(ev.targetId)
			};
			case "blur": return {
				kind: "blur",
				target: asElementId(ev.targetId)
			};
			case "text_input": return {
				kind: "input",
				target: asElementId(ev.targetId),
				value: ev.text
			};
			case "hover_enter": return {
				kind: "hover-enter",
				target: asElementId(ev.targetId)
			};
			case "hover_leave": return {
				kind: "hover-leave",
				target: asElementId(ev.targetId)
			};
			case "key_down": return {
				kind: "keydown",
				target: asElementId(ev.targetId),
				key: ev.key
			};
			default: return null;
		}
	}
	//#endregion
	//#region ../../packages/renderer-canvas/dist/chunk-7ZONU764.js
	function viewportAxis(value) {
		return value === void 0 ? -1 : value;
	}
	function splitStyleVariant(style) {
		const split = [];
		for (const key in style) {
			const k = key;
			if (style[k] === void 0) continue;
			split.push({ [k]: style[k] });
		}
		return split;
	}
	function encodeMutations(mutations) {
		const ops = [];
		const styles = [];
		const texts = [];
		for (const mutation of mutations) switch (mutation.kind) {
			case "createElement":
				appendCreate(ops, mutation.id, ELEMENT_KIND[mutation.elementKind]);
				break;
			case "setRoot":
				appendSetRoot(ops, mutation.id);
				break;
			case "appendChild":
				appendChild(ops, mutation.parent, mutation.child);
				break;
			case "insertBefore":
				insertBefore(ops, mutation.parent, mutation.child, mutation.before);
				break;
			case "remove":
				appendRemove(ops, mutation.id);
				break;
			case "setStyle": {
				const offset = styles.length;
				encodeStylePatch(mutation.style, styles);
				const len = styles.length - offset;
				if (len > 0) appendSetStyle(ops, mutation.id, offset, len);
				for (const unsetKind of unsetKindsOf(mutation.style)) appendUnsetStyle(ops, mutation.id, unsetKind);
				break;
			}
			case "setText": {
				const textIndex = texts.length;
				texts.push(mutation.text);
				appendSetText(ops, mutation.id, textIndex);
				break;
			}
			case "setTextContent": {
				const textIndex = texts.length;
				texts.push(mutation.text);
				appendSetTextContent(ops, mutation.id, textIndex);
				break;
			}
			case "setDisabled":
				appendSetDisabled(ops, mutation.id, mutation.disabled ? 1 : 0);
				break;
			case "setUserSelect":
				appendSetUserSelect(ops, mutation.id, USER_SELECT[mutation.value]);
				break;
			case "setMultiline":
				appendSetMultiline(ops, mutation.id, mutation.multiline ? 1 : 0);
				break;
			case "setSrc": {
				const textIndex = texts.length;
				texts.push(mutation.url);
				appendSetSrc(ops, mutation.id, textIndex);
				break;
			}
			case "setPseudoStyle": {
				const offset = styles.length;
				encodeStylePatch(mutation.style, styles);
				const len = styles.length - offset;
				if (len > 0) appendSetPseudoStyle(ops, mutation.id, PSEUDO_STATE_CODE[mutation.pseudo], offset, len);
				break;
			}
			case "setStyleVariant":
				for (const single of splitStyleVariant(mutation.style)) {
					const offset = styles.length;
					encodeStylePatch(single, styles);
					const len = styles.length - offset;
					if (len > 0) appendSetStyleVariant(ops, mutation.id, viewportAxis(mutation.condition.minWidth), viewportAxis(mutation.condition.maxWidth), viewportAxis(mutation.condition.minHeight), viewportAxis(mutation.condition.maxHeight), offset, len);
				}
				break;
		}
		return {
			ops: new Float64Array(ops),
			styles: new Float32Array(styles),
			texts
		};
	}
	var HayateMutationPacket = class {
		constructor() {
			_defineProperty(this, "mutations", []);
		}
		enqueueCreateElement(id, kind) {
			this.mutations.push({
				kind: "createElement",
				id,
				elementKind: kind
			});
		}
		enqueueSetRoot(id) {
			this.mutations.push({
				kind: "setRoot",
				id
			});
		}
		enqueueAppendChild(parent, child) {
			this.mutations.push({
				kind: "appendChild",
				parent,
				child
			});
		}
		enqueueInsertBefore(parent, child, before) {
			this.mutations.push({
				kind: "insertBefore",
				parent,
				child,
				before
			});
		}
		enqueueRemove(id) {
			this.mutations.push({
				kind: "remove",
				id
			});
		}
		enqueueSetStyle(id, style) {
			this.mutations.push({
				kind: "setStyle",
				id,
				style: { ...style }
			});
		}
		enqueueSetText(id, text) {
			this.mutations.push({
				kind: "setText",
				id,
				text
			});
		}
		enqueueSetTextContent(id, text) {
			this.mutations.push({
				kind: "setTextContent",
				id,
				text
			});
		}
		enqueueSetDisabled(id, disabled) {
			this.mutations.push({
				kind: "setDisabled",
				id,
				disabled
			});
		}
		enqueueSetUserSelect(id, value) {
			this.mutations.push({
				kind: "setUserSelect",
				id,
				value
			});
		}
		enqueueSetMultiline(id, multiline) {
			this.mutations.push({
				kind: "setMultiline",
				id,
				multiline
			});
		}
		enqueueSetSrc(id, url) {
			this.mutations.push({
				kind: "setSrc",
				id,
				url
			});
		}
		enqueueSetPseudoStyle(id, pseudo, style) {
			this.mutations.push({
				kind: "setPseudoStyle",
				id,
				pseudo,
				style: { ...style }
			});
		}
		enqueueSetStyleVariant(id, condition, style) {
			this.mutations.push({
				kind: "setStyleVariant",
				id,
				condition,
				style: { ...style }
			});
		}
		flush(raw) {
			if (this.mutations.length === 0) return;
			const { ops, styles, texts } = encodeMutations(this.mutations);
			if (ops.length > 0) raw.apply_mutations(ops, styles, texts);
			this.mutations.length = 0;
		}
	};
	new TextEncoder();
	function canvasPixelRectToDomRect(canvas, x, y, width, height) {
		const rect = canvas.getBoundingClientRect();
		const scaleX = canvas.width === 0 ? 1 : rect.width / canvas.width;
		const scaleY = canvas.height === 0 ? 1 : rect.height / canvas.height;
		return new DOMRect(rect.left + x * scaleX, rect.top + y * scaleY, width * scaleX, height * scaleY);
	}
	var editContexts = /* @__PURE__ */ new WeakMap();
	function syncEditContext(canvas, raw) {
		const wants = raw.ime_wants_keyboard();
		const owned = editContexts.get(canvas);
		if (owned !== void 0) {
			if (wants) {
				if (canvas.editContext !== owned) canvas.editContext = owned;
			} else if (canvas.editContext === owned) canvas.editContext = null;
		}
		if (!wants) return;
		const editContext = canvas.editContext;
		if (editContext === void 0 || editContext === null) return;
		const bounds = raw.ime_character_bounds();
		if (bounds[2] === 0 && bounds[3] === 0) return;
		const dom = canvasPixelRectToDomRect(canvas, bounds[0], bounds[1], bounds[2], bounds[3]);
		editContext.updateControlBounds(dom);
		editContext.updateSelectionBounds(dom);
	}
	var CanvasRenderer = class {
		constructor(raw, options = {}) {
			_defineProperty(this, "raw", void 0);
			_defineProperty(
				this,
				/** Hayate が発行したリスナ id → ホストのハンドラ（ADR-0053）。 */
				"listeners",
				/* @__PURE__ */ new Map()
			);
			_defineProperty(this, "nextId", 1);
			_defineProperty(this, "packet", new HayateMutationPacket());
			_defineProperty(this, "canvas", void 0);
			_defineProperty(this, "requestFrame", void 0);
			_defineProperty(this, "cancelFrame", void 0);
			_defineProperty(
				this,
				/** DPR の明示上書き（テスト/埋め込みホスト）。未設定なら毎リサイズで実時の
				* `globalThis.devicePixelRatio` を読む。モバイル Chrome は構築後に DPR を変える
				* （入力中のソフトキーボード/フォーカスズーム）ため、構築時にキャッシュすると
				* バッキングストアが小さすぎて生成され、シーンが拡大されてグリフが粗くなる。 */
				"devicePixelRatioOverride",
				void 0
			);
			_defineProperty(this, "resizeObserver", null);
			_defineProperty(this, "frameHandle", null);
			_defineProperty(this, "frame", (timestampMs) => {
				this.flush();
				this.raw.render(timestampMs);
				if (this.canvas !== null) syncEditContext(this.canvas, this.raw);
				this.dispatchDeliveries(this.raw.poll_events());
				this.frameHandle = this.requestFrame(this.frame);
			});
			this.raw = raw;
			this.canvas = options.canvas ?? null;
			this.requestFrame = options.requestFrame ?? globalThis.requestAnimationFrame.bind(globalThis);
			this.cancelFrame = options.cancelFrame ?? globalThis.cancelAnimationFrame.bind(globalThis);
			this.devicePixelRatioOverride = options.devicePixelRatio;
			const autoResize = options.autoResize ?? this.canvas !== null;
			if (this.canvas !== null && autoResize) this.attachResizeObserver(this.canvas, options.createResizeObserver);
			this.frameHandle = this.requestFrame(this.frame);
		}
		stop() {
			if (this.frameHandle !== null) {
				this.cancelFrame(this.frameHandle);
				this.frameHandle = null;
			}
			this.resizeObserver?.disconnect();
			this.resizeObserver = null;
		}
		attachResizeObserver(canvas, createResizeObserver) {
			const ResizeObserverCtor = createResizeObserver ?? (typeof globalThis.ResizeObserver !== "undefined" ? globalThis.ResizeObserver : void 0);
			if (ResizeObserverCtor === void 0) return;
			const syncFromContentBox = (width, height) => {
				this.resize(Math.round(width), Math.round(height), this.currentDevicePixelRatio());
			};
			const rect = canvas.getBoundingClientRect();
			syncFromContentBox(rect.width, rect.height);
			const observer = new ResizeObserverCtor((entries) => {
				const entry = entries[0];
				if (entry === void 0) return;
				const { width, height } = entry.contentRect;
				syncFromContentBox(width, height);
			});
			observer.observe(canvas);
			this.resizeObserver = observer;
		}
		/** 次のリサイズに使う DPR を決める。明示上書きがあればそれを、なければ実時の
		* グローバル値（毎回読み直し、キャッシュしない）。 */
		currentDevicePixelRatio() {
			return this.devicePixelRatioOverride ?? globalThis.devicePixelRatio ?? 1;
		}
		resize(width, height, scale = 1) {
			const dpr = Math.max(1, scale);
			if (this.canvas !== null) {
				this.canvas.width = Math.round(width * dpr);
				this.canvas.height = Math.round(height * dpr);
			}
			this.raw.on_resize(width, height, dpr);
		}
		createElement(kind) {
			const id = asElementId(this.nextId++);
			this.packet.enqueueCreateElement(id, kind);
			return id;
		}
		setRoot(id) {
			this.packet.enqueueSetRoot(id);
		}
		appendChild(parent, child) {
			this.packet.enqueueAppendChild(parent, child);
		}
		insertBefore(parent, child, before) {
			this.packet.enqueueInsertBefore(parent, child, before);
		}
		removeChild(_parent, child) {
			this.packet.enqueueRemove(child);
		}
		setStyle(id, style) {
			this.packet.enqueueSetStyle(id, style);
		}
		setPseudoStyle(id, pseudo, style) {
			this.packet.enqueueSetPseudoStyle(id, pseudo, style);
		}
		setStyleVariant(id, condition, style) {
			this.packet.enqueueSetStyleVariant(id, condition, style);
		}
		setText(id, text) {
			this.packet.enqueueSetText(id, text);
		}
		setProperty(id, name, value) {
			assertKnownElementProperty(name);
			dispatchElementPropertyOp(coerceElementProperty(name, value), {
				"text-content": ({ text }) => this.packet.enqueueSetTextContent(id, text),
				placeholder: ({ text }) => this.packet.enqueueSetText(id, text),
				src: ({ text }) => this.packet.enqueueSetSrc(id, text),
				disabled: ({ disabled }) => this.packet.enqueueSetDisabled(id, disabled),
				"user-select": ({ value: value2 }) => this.packet.enqueueSetUserSelect(id, value2),
				multiline: ({ multiline }) => this.packet.enqueueSetMultiline(id, multiline)
			});
		}
		addEventListener(id, event, handler) {
			const hayateKind = HAYATE_LISTENER_KIND[event];
			if (hayateKind === void 0) return () => {};
			const listenerId = this.raw.register_listener(id, hayateKind);
			this.listeners.set(listenerId, {
				handler,
				elementId: id
			});
			return () => {
				this.listeners.delete(listenerId);
			};
		}
		/** 順序付きミューテーションパケットを Hayate WASM 境界へ流し込む。 */
		flush() {
			this.packet.flush(this.raw);
		}
		dispatchDeliveries(rows) {
			for (const row of rows) {
				const { listenerId, event } = parseDelivery(row);
				const entry = this.listeners.get(listenerId);
				if (entry === void 0) continue;
				const interaction = toInteractionEvent(event);
				if (interaction !== null) {
					if (interaction.kind === "input") interaction.value = this.raw.element_get_text_content(interaction.target);
					entry.handler(interaction);
				}
			}
		}
	};
	//#endregion
	//#region ../../packages/renderer-canvas/dist/android.js
	function createAndroidCanvasRenderer(raw, options) {
		let pendingFrame = null;
		let handleSeq = 1;
		const requestFrame = (cb) => {
			pendingFrame = cb;
			return handleSeq++;
		};
		const cancelFrame = (_handle) => {
			pendingFrame = null;
		};
		const renderer = new CanvasRenderer(raw, {
			...options,
			requestFrame,
			cancelFrame
		});
		return {
			renderer,
			pumpFrame(timestampMs) {
				const cb = pendingFrame;
				pendingFrame = null;
				cb?.(timestampMs);
			},
			resize(width, height, scale = 1) {
				renderer.resize(width, height, scale);
			},
			stop() {
				pendingFrame = null;
				renderer.stop();
			}
		};
	}
	//#endregion
	//#region src/theme.ts
	/** スウォッチに並べるアクセント色の順序（UI と検証で共有する正本）。 */
	var ACCENT_KEYS = [
		"teal",
		"pink",
		"orange",
		"lime",
		"violet"
	];
	/** 既定はライト（gomi 準拠）。 */
	var DEFAULT_THEME = "light";
	/** 既定アクセントは teal（従来デモの基調色）。 */
	var DEFAULT_ACCENT = "teal";
	var LIGHT_BASE = {
		bg: "#f1ede3",
		rail: "#fbf8f1",
		panel: "#fdfdfb",
		panel2: "#ece6d8",
		panel3: "#e0d8c7",
		ink: "#262130",
		text: "#322c3f",
		muted: "#6f6878",
		quiet: "#9a93a3",
		line: "#d9d3c6",
		accent2: "#ef9d2e",
		danger: "#e5484d",
		dangerBg: "#fbe4e4",
		success: "#2fa86a",
		successBg: "#d8f0e2",
		blue: "#4b8ef0",
		violet: "#8b5cf6",
		black: "#14101c",
		shadow: "#2621301f"
	};
	var DARK_BASE = {
		bg: "#0b1020",
		rail: "#111827",
		panel: "#162033",
		panel2: "#1b2a3f",
		panel3: "#21344e",
		ink: "#eef4ff",
		text: "#d8e2f2",
		muted: "#8ea1bb",
		quiet: "#5f728d",
		line: "#31425b",
		accent2: "#f59e0b",
		danger: "#fb7185",
		dangerBg: "#3d1722",
		success: "#65d38c",
		successBg: "#163526",
		blue: "#60a5fa",
		violet: "#a78bfa",
		black: "#070b14",
		shadow: "#00000066"
	};
	/** 各アクセントのテーマ別 hex。明色は dark、彩度を上げた版は light で読みやすいよう分ける。 */
	var ACCENT_SWATCHES = {
		teal: {
			light: "#14b8a6",
			dark: "#4fd1c5"
		},
		pink: {
			light: "#e84d8a",
			dark: "#f472b6"
		},
		orange: {
			light: "#ef8f3c",
			dark: "#fb923c"
		},
		lime: {
			light: "#5ca80f",
			dark: "#a3e635"
		},
		violet: {
			light: "#7c5cf0",
			dark: "#a78bfa"
		}
	};
	/** ライト/ダーク × アクセント色から全色を解決する。 */
	function palette(theme, accent) {
		return {
			...theme === "dark" ? DARK_BASE : LIGHT_BASE,
			accent: ACCENT_SWATCHES[accent][theme]
		};
	}
	/** スウォッチ表示用に、現在テーマでのアクセント色を返す。 */
	function accentColor(theme, accent) {
		return ACCENT_SWATCHES[accent][theme];
	}
	/** text-input の基本スタイル。パレットから色を解決する。 */
	function inputStyle(p) {
		return {
			height: 38,
			paddingLeft: 12,
			paddingRight: 12,
			backgroundColor: p.panel2,
			color: p.text,
			borderRadius: 8,
			borderWidth: 1,
			borderStyle: "solid",
			borderColor: p.line,
			fontSize: 13,
			transitionDuration: 160,
			transitionTiming: "ease-out",
			":focus": {
				borderColor: p.accent,
				backgroundColor: p.panel3,
				boxShadow: [{
					offsetX: 0,
					offsetY: 0,
					blur: 0,
					spread: 3,
					color: `${p.accent}33`,
					inset: false
				}]
			}
		};
	}
	/** localStorage に書き込む既定のキー（#247 の永続化方針に合わせる）。 */
	var THEME_STORAGE_KEY = "pop-theme-v1";
	var DEFAULT_PREFS = {
		theme: DEFAULT_THEME,
		accent: DEFAULT_ACCENT
	};
	function isTheme(value) {
		return value === "light" || value === "dark";
	}
	function isAccent(value) {
		return typeof value === "string" && ACCENT_KEYS.includes(value);
	}
	/** テーマ設定を保存用文字列へ変換する。 */
	function serializeTheme(prefs) {
		return JSON.stringify(prefs);
	}
	/**
	* 保存文字列をテーマ設定へ復元する。
	* null・不正 JSON・形が壊れている・未知のテーマ/アクセントは既定（ライト/teal）へフォールバック。
	*/
	function deserializeTheme(raw) {
		if (raw === null) return { ...DEFAULT_PREFS };
		try {
			const parsed = JSON.parse(raw);
			if (typeof parsed === "object" && parsed !== null) {
				const value = parsed;
				if (isTheme(value.theme) && isAccent(value.accent)) return {
					theme: value.theme,
					accent: value.accent
				};
			}
		} catch {}
		return { ...DEFAULT_PREFS };
	}
	/** ストレージからテーマ設定を読み込む（無い/壊れていれば既定）。 */
	function loadTheme(storage, key = THEME_STORAGE_KEY) {
		return deserializeTheme(storage.getItem(key));
	}
	/** テーマ設定をストレージへ保存する。 */
	function saveTheme(storage, prefs, key = THEME_STORAGE_KEY) {
		storage.setItem(key, serializeTheme(prefs));
	}
	//#endregion
	//#region src/gallery/sections.tsx
	/**
	* `@media` ブレークポイントのライブ実証（ADR-0081）。Hayate CSS には
	* スタイルシートが無いため、media は raw CSS ではなく `styleVariants` という
	* 型付き宣言で要素ごとに載せる。DOM Renderer ではこれが本物の
	* `@media (min-width: …)` ルールにコンパイルされ（DevTools の
	* `<style data-tsubame-variant>` で確認できる）、Canvas Renderer では viewport で
	* 評価される。ウィンドウ幅を変えると、現在マッチする帯のタイルだけが点灯する。
	*
	* 帯は元デモ（gomi/todo-demo-v2.css の `.mq-tile`）と同じ S(<720) / M(720–1099) /
	* L(≥1100) の 3 段。各タイルは base が `muted`、自帯の variant でだけ `accent` に
	* なる。`defaultColor` は ambient チャネルなので子 `text` まで継承する。
	*/
	var MQ_TILES = [
		{
			label: "S  < 720",
			condition: { maxWidth: 719 }
		},
		{
			label: "M  720–1099",
			condition: {
				minWidth: 720,
				maxWidth: 1099
			}
		},
		{
			label: "L  ≥ 1100",
			condition: { minWidth: 1100 }
		}
	];
	function MediaTiles(props) {
		const p = props.colors;
		return (() => {
			var _el$ = createElement("view");
			setProp(_el$, "style", {
				display: "flex",
				flexDirection: "column",
				gap: 6,
				width: 200
			});
			insert(_el$, () => MQ_TILES.map((tile) => (() => {
				var _el$2 = createElement("view"), _el$3 = createElement("text");
				insertNode(_el$2, _el$3);
				insert(_el$3, () => tile.label);
				effect((_p$) => {
					var _v$ = {
						height: 34,
						display: "flex",
						alignItems: "center",
						justifyContent: "center",
						backgroundColor: p.panel2,
						defaultColor: p.muted,
						defaultFontSize: 12,
						borderRadius: 8,
						borderWidth: 1,
						borderStyle: "solid",
						borderColor: p.line
					}, _v$2 = [{
						condition: tile.condition,
						style: {
							backgroundColor: p.accent,
							defaultColor: p.black,
							borderColor: p.accent
						}
					}];
					_v$ !== _p$.e && (_p$.e = setProp(_el$2, "style", _v$, _p$.e));
					_v$2 !== _p$.t && (_p$.t = setProp(_el$2, "styleVariants", _v$2, _p$.t));
					return _p$;
				}, {
					e: void 0,
					t: void 0
				});
				return _el$2;
			})()));
			return _el$;
		})();
	}
	function SampleBox(props) {
		return (() => {
			var _el$4 = createElement("view"), _el$5 = createElement("text");
			insertNode(_el$4, _el$5);
			insert(_el$5, () => props.label);
			effect((_p$) => {
				var _v$3 = {
					width: 120,
					height: 56,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					backgroundColor: props.colors.panel2,
					borderWidth: 1,
					borderColor: props.colors.line,
					borderRadius: 10,
					...props.style
				}, _v$4 = {
					color: props.colors.text,
					fontSize: 12
				};
				_v$3 !== _p$.e && (_p$.e = setProp(_el$4, "style", _v$3, _p$.e));
				_v$4 !== _p$.t && (_p$.t = setProp(_el$5, "style", _v$4, _p$.t));
				return _p$;
			}, {
				e: void 0,
				t: void 0
			});
			return _el$4;
		})();
	}
	function buildSections(p) {
		return [
			{
				title: "Visual",
				accent: p.accent,
				cards: [
					{
						title: "backgroundColor",
						properties: ["backgroundColor"],
						render: () => createComponent(SampleBox, {
							colors: p,
							label: "Sample",
							get style() {
								return { backgroundColor: p.accent };
							}
						})
					},
					{
						title: "opacity",
						properties: ["opacity"],
						render: () => createComponent(SampleBox, {
							colors: p,
							label: "0.45",
							style: { opacity: .45 }
						})
					},
					{
						title: "borderRadius",
						properties: ["borderRadius"],
						render: () => createComponent(SampleBox, {
							colors: p,
							label: "r16",
							style: { borderRadius: 16 }
						})
					},
					{
						title: "borderWidth",
						properties: ["borderWidth"],
						render: () => createComponent(SampleBox, {
							colors: p,
							label: "3px",
							get style() {
								return {
									borderWidth: 3,
									borderColor: p.accent
								};
							}
						})
					},
					{
						title: "borderColor",
						properties: ["borderColor"],
						render: () => createComponent(SampleBox, {
							colors: p,
							label: "violet",
							get style() {
								return {
									borderWidth: 2,
									borderColor: p.violet
								};
							}
						})
					},
					{
						title: "borderStyle",
						properties: ["borderStyle"],
						note: "solid / dashed",
						render: () => (() => {
							var _el$6 = createElement("view");
							setProp(_el$6, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 6
							});
							insert(_el$6, createComponent(SampleBox, {
								colors: p,
								label: "solid",
								get style() {
									return {
										borderWidth: 2,
										borderStyle: "solid",
										borderColor: p.accent
									};
								}
							}), null);
							insert(_el$6, createComponent(SampleBox, {
								colors: p,
								label: "dashed",
								get style() {
									return {
										borderWidth: 2,
										borderStyle: "dashed",
										borderColor: p.accent2
									};
								}
							}), null);
							return _el$6;
						})()
					},
					{
						title: "boxShadow",
						properties: ["boxShadow"],
						note: "elevation + inset ring — ADR-0095",
						render: () => (() => {
							var _el$7 = createElement("view");
							setProp(_el$7, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 10,
								padding: 6
							});
							insert(_el$7, createComponent(SampleBox, {
								colors: p,
								label: "lift",
								get style() {
									return { boxShadow: [{
										offsetX: 0,
										offsetY: 6,
										blur: 16,
										spread: 0,
										color: p.shadow,
										inset: false
									}] };
								}
							}), null);
							insert(_el$7, createComponent(SampleBox, {
								colors: p,
								label: "inset",
								get style() {
									return { boxShadow: [{
										offsetX: 0,
										offsetY: 0,
										blur: 0,
										spread: 3,
										color: p.accent,
										inset: true
									}] };
								}
							}), null);
							return _el$7;
						})()
					}
				]
			},
			{
				title: "Sizing",
				accent: p.blue,
				cards: [
					["width", { width: 140 }],
					["height", { height: 72 }],
					["minWidth", {
						minWidth: 120,
						width: 80
					}],
					["minHeight", {
						minHeight: 64,
						height: 40
					}],
					["maxWidth", {
						maxWidth: 90,
						width: 140
					}],
					["maxHeight", {
						maxHeight: 40,
						height: 72
					}]
				].map(([name, style]) => ({
					title: name,
					properties: [name],
					render: () => createComponent(SampleBox, {
						colors: p,
						label: "Sample",
						style
					})
				}))
			},
			{
				title: "Spacing",
				accent: p.violet,
				cards: [
					...[
						"padding",
						"paddingTop",
						"paddingRight",
						"paddingBottom",
						"paddingLeft"
					].map((key) => ({
						title: key,
						properties: [key],
						render: () => (() => {
							var _el$8 = createElement("view"), _el$9 = createElement("view");
							insertNode(_el$8, _el$9);
							effect((_p$) => {
								var _v$5 = {
									backgroundColor: p.panel2,
									borderWidth: 1,
									borderColor: p.line,
									borderRadius: 8,
									[key]: 14
								}, _v$6 = {
									backgroundColor: p.accent,
									height: 28,
									width: 80,
									borderRadius: 6
								};
								_v$5 !== _p$.e && (_p$.e = setProp(_el$8, "style", _v$5, _p$.e));
								_v$6 !== _p$.t && (_p$.t = setProp(_el$9, "style", _v$6, _p$.t));
								return _p$;
							}, {
								e: void 0,
								t: void 0
							});
							return _el$8;
						})()
					})),
					...[
						"margin",
						"marginTop",
						"marginRight",
						"marginBottom",
						"marginLeft"
					].map((key) => ({
						title: key,
						properties: [key],
						render: () => (() => {
							var _el$0 = createElement("view"), _el$1 = createElement("view");
							insertNode(_el$0, _el$1);
							effect((_p$) => {
								var _v$7 = {
									backgroundColor: p.black,
									padding: 4,
									borderRadius: 8
								}, _v$8 = {
									backgroundColor: p.panel2,
									borderWidth: 1,
									borderColor: p.line,
									borderRadius: 6,
									height: 28,
									width: 80,
									[key]: 10
								};
								_v$7 !== _p$.e && (_p$.e = setProp(_el$0, "style", _v$7, _p$.e));
								_v$8 !== _p$.t && (_p$.t = setProp(_el$1, "style", _v$8, _p$.t));
								return _p$;
							}, {
								e: void 0,
								t: void 0
							});
							return _el$0;
						})()
					})),
					{
						title: "gap",
						properties: ["gap"],
						render: () => (() => {
							var _el$10 = createElement("view"), _el$11 = createElement("view"), _el$12 = createElement("view");
							insertNode(_el$10, _el$11);
							insertNode(_el$10, _el$12);
							effect((_p$) => {
								var _v$9 = {
									display: "flex",
									flexDirection: "row",
									gap: 16,
									backgroundColor: p.panel2,
									padding: 8,
									borderRadius: 8,
									borderWidth: 1,
									borderColor: p.line
								}, _v$0 = {
									width: 36,
									height: 24,
									backgroundColor: p.accent,
									borderRadius: 6
								}, _v$1 = {
									width: 36,
									height: 24,
									backgroundColor: p.blue,
									borderRadius: 6
								};
								_v$9 !== _p$.e && (_p$.e = setProp(_el$10, "style", _v$9, _p$.e));
								_v$0 !== _p$.t && (_p$.t = setProp(_el$11, "style", _v$0, _p$.t));
								_v$1 !== _p$.a && (_p$.a = setProp(_el$12, "style", _v$1, _p$.a));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0
							});
							return _el$10;
						})()
					}
				]
			},
			{
				title: "Flex & Grid",
				accent: p.accent2,
				cards: [
					{
						title: "display",
						properties: ["display"],
						render: () => (() => {
							var _el$13 = createElement("view"), _el$14 = createElement("view"), _el$15 = createElement("view");
							insertNode(_el$13, _el$14);
							insertNode(_el$13, _el$15);
							setProp(_el$13, "style", {
								display: "flex",
								flexDirection: "row",
								gap: 6
							});
							effect((_p$) => {
								var _v$10 = {
									width: 24,
									height: 24,
									backgroundColor: p.accent,
									borderRadius: 6
								}, _v$11 = {
									width: 24,
									height: 24,
									backgroundColor: p.blue,
									borderRadius: 6
								};
								_v$10 !== _p$.e && (_p$.e = setProp(_el$14, "style", _v$10, _p$.e));
								_v$11 !== _p$.t && (_p$.t = setProp(_el$15, "style", _v$11, _p$.t));
								return _p$;
							}, {
								e: void 0,
								t: void 0
							});
							return _el$13;
						})()
					},
					{
						title: "flexDirection",
						properties: ["flexDirection"],
						render: () => (() => {
							var _el$16 = createElement("view"), _el$17 = createElement("view"), _el$18 = createElement("view");
							insertNode(_el$16, _el$17);
							insertNode(_el$16, _el$18);
							setProp(_el$16, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 6,
								height: 72
							});
							effect((_p$) => {
								var _v$12 = {
									width: 48,
									height: 16,
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$13 = {
									width: 48,
									height: 16,
									backgroundColor: p.blue,
									borderRadius: 4
								};
								_v$12 !== _p$.e && (_p$.e = setProp(_el$17, "style", _v$12, _p$.e));
								_v$13 !== _p$.t && (_p$.t = setProp(_el$18, "style", _v$13, _p$.t));
								return _p$;
							}, {
								e: void 0,
								t: void 0
							});
							return _el$16;
						})()
					},
					{
						title: "flexWrap",
						properties: ["flexWrap"],
						render: () => (() => {
							var _el$19 = createElement("view"), _el$20 = createElement("view"), _el$21 = createElement("view"), _el$22 = createElement("view");
							insertNode(_el$19, _el$20);
							insertNode(_el$19, _el$21);
							insertNode(_el$19, _el$22);
							setProp(_el$19, "style", {
								display: "flex",
								flexWrap: "wrap",
								width: 120,
								gap: 4
							});
							effect((_p$) => {
								var _v$14 = {
									width: 48,
									height: 20,
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$15 = {
									width: 48,
									height: 20,
									backgroundColor: p.blue,
									borderRadius: 4
								}, _v$16 = {
									width: 48,
									height: 20,
									backgroundColor: p.violet,
									borderRadius: 4
								};
								_v$14 !== _p$.e && (_p$.e = setProp(_el$20, "style", _v$14, _p$.e));
								_v$15 !== _p$.t && (_p$.t = setProp(_el$21, "style", _v$15, _p$.t));
								_v$16 !== _p$.a && (_p$.a = setProp(_el$22, "style", _v$16, _p$.a));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0
							});
							return _el$19;
						})()
					},
					{
						title: "alignItems",
						properties: ["alignItems"],
						render: () => (() => {
							var _el$23 = createElement("view"), _el$24 = createElement("view"), _el$25 = createElement("view");
							insertNode(_el$23, _el$24);
							insertNode(_el$23, _el$25);
							effect((_p$) => {
								var _v$17 = {
									display: "flex",
									flexDirection: "row",
									alignItems: "center",
									gap: 6,
									height: 56,
									backgroundColor: p.panel2,
									borderRadius: 8
								}, _v$18 = {
									width: 20,
									height: 20,
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$19 = {
									width: 20,
									height: 36,
									backgroundColor: p.blue,
									borderRadius: 4
								};
								_v$17 !== _p$.e && (_p$.e = setProp(_el$23, "style", _v$17, _p$.e));
								_v$18 !== _p$.t && (_p$.t = setProp(_el$24, "style", _v$18, _p$.t));
								_v$19 !== _p$.a && (_p$.a = setProp(_el$25, "style", _v$19, _p$.a));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0
							});
							return _el$23;
						})()
					},
					{
						title: "justifyContent",
						properties: ["justifyContent"],
						render: () => (() => {
							var _el$26 = createElement("view"), _el$27 = createElement("view"), _el$28 = createElement("view");
							insertNode(_el$26, _el$27);
							insertNode(_el$26, _el$28);
							effect((_p$) => {
								var _v$20 = {
									display: "flex",
									flexDirection: "row",
									justifyContent: "space-between",
									width: 140,
									backgroundColor: p.panel2,
									borderRadius: 8
								}, _v$21 = {
									width: 20,
									height: 20,
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$22 = {
									width: 20,
									height: 20,
									backgroundColor: p.blue,
									borderRadius: 4
								};
								_v$20 !== _p$.e && (_p$.e = setProp(_el$26, "style", _v$20, _p$.e));
								_v$21 !== _p$.t && (_p$.t = setProp(_el$27, "style", _v$21, _p$.t));
								_v$22 !== _p$.a && (_p$.a = setProp(_el$28, "style", _v$22, _p$.a));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0
							});
							return _el$26;
						})()
					},
					{
						title: "flexGrow",
						properties: ["flexGrow"],
						render: () => (() => {
							var _el$29 = createElement("view"), _el$30 = createElement("view"), _el$31 = createElement("view");
							insertNode(_el$29, _el$30);
							insertNode(_el$29, _el$31);
							setProp(_el$29, "style", {
								display: "flex",
								flexDirection: "row",
								width: 140,
								gap: 4
							});
							effect((_p$) => {
								var _v$23 = {
									flexGrow: 1,
									height: 24,
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$24 = {
									width: 24,
									height: 24,
									backgroundColor: p.blue,
									borderRadius: 4
								};
								_v$23 !== _p$.e && (_p$.e = setProp(_el$30, "style", _v$23, _p$.e));
								_v$24 !== _p$.t && (_p$.t = setProp(_el$31, "style", _v$24, _p$.t));
								return _p$;
							}, {
								e: void 0,
								t: void 0
							});
							return _el$29;
						})()
					},
					{
						title: "flexShrink",
						properties: ["flexShrink"],
						render: () => (() => {
							var _el$32 = createElement("view"), _el$33 = createElement("view"), _el$34 = createElement("view");
							insertNode(_el$32, _el$33);
							insertNode(_el$32, _el$34);
							setProp(_el$32, "style", {
								display: "flex",
								flexDirection: "row",
								width: 100,
								gap: 4
							});
							effect((_p$) => {
								var _v$25 = {
									width: 80,
									flexShrink: 2,
									height: 24,
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$26 = {
									width: 80,
									flexShrink: 0,
									height: 24,
									backgroundColor: p.blue,
									borderRadius: 4
								};
								_v$25 !== _p$.e && (_p$.e = setProp(_el$33, "style", _v$25, _p$.e));
								_v$26 !== _p$.t && (_p$.t = setProp(_el$34, "style", _v$26, _p$.t));
								return _p$;
							}, {
								e: void 0,
								t: void 0
							});
							return _el$32;
						})()
					},
					{
						title: "flexBasis",
						properties: ["flexBasis"],
						render: () => (() => {
							var _el$35 = createElement("view"), _el$36 = createElement("view"), _el$37 = createElement("view");
							insertNode(_el$35, _el$36);
							insertNode(_el$35, _el$37);
							setProp(_el$35, "style", {
								display: "flex",
								flexDirection: "row",
								width: 140,
								gap: 4
							});
							effect((_p$) => {
								var _v$27 = {
									flexBasis: 60,
									height: 24,
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$28 = {
									flexGrow: 1,
									height: 24,
									backgroundColor: p.blue,
									borderRadius: 4
								};
								_v$27 !== _p$.e && (_p$.e = setProp(_el$36, "style", _v$27, _p$.e));
								_v$28 !== _p$.t && (_p$.t = setProp(_el$37, "style", _v$28, _p$.t));
								return _p$;
							}, {
								e: void 0,
								t: void 0
							});
							return _el$35;
						})()
					},
					{
						title: "alignSelf",
						properties: ["alignSelf"],
						render: () => (() => {
							var _el$38 = createElement("view"), _el$39 = createElement("view"), _el$40 = createElement("view");
							insertNode(_el$38, _el$39);
							insertNode(_el$38, _el$40);
							effect((_p$) => {
								var _v$29 = {
									display: "flex",
									flexDirection: "row",
									alignItems: "flex-start",
									gap: 6,
									height: 56,
									backgroundColor: p.panel2,
									borderRadius: 8
								}, _v$30 = {
									width: 20,
									height: 20,
									backgroundColor: p.muted,
									borderRadius: 4
								}, _v$31 = {
									width: 20,
									height: 36,
									alignSelf: "flex-end",
									backgroundColor: p.accent,
									borderRadius: 4
								};
								_v$29 !== _p$.e && (_p$.e = setProp(_el$38, "style", _v$29, _p$.e));
								_v$30 !== _p$.t && (_p$.t = setProp(_el$39, "style", _v$30, _p$.t));
								_v$31 !== _p$.a && (_p$.a = setProp(_el$40, "style", _v$31, _p$.a));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0
							});
							return _el$38;
						})()
					},
					{
						title: "alignContent",
						properties: ["alignContent"],
						render: () => (() => {
							var _el$41 = createElement("view"), _el$42 = createElement("view"), _el$43 = createElement("view"), _el$44 = createElement("view"), _el$45 = createElement("view");
							insertNode(_el$41, _el$42);
							insertNode(_el$41, _el$43);
							insertNode(_el$41, _el$44);
							insertNode(_el$41, _el$45);
							effect((_p$) => {
								var _v$32 = {
									display: "flex",
									flexWrap: "wrap",
									alignContent: "space-between",
									width: 100,
									height: 72,
									gap: 4,
									backgroundColor: p.panel2,
									borderRadius: 8
								}, _v$33 = {
									width: 40,
									height: 20,
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$34 = {
									width: 40,
									height: 20,
									backgroundColor: p.blue,
									borderRadius: 4
								}, _v$35 = {
									width: 40,
									height: 20,
									backgroundColor: p.violet,
									borderRadius: 4
								}, _v$36 = {
									width: 40,
									height: 20,
									backgroundColor: p.accent,
									borderRadius: 4
								};
								_v$32 !== _p$.e && (_p$.e = setProp(_el$41, "style", _v$32, _p$.e));
								_v$33 !== _p$.t && (_p$.t = setProp(_el$42, "style", _v$33, _p$.t));
								_v$34 !== _p$.a && (_p$.a = setProp(_el$43, "style", _v$34, _p$.a));
								_v$35 !== _p$.o && (_p$.o = setProp(_el$44, "style", _v$35, _p$.o));
								_v$36 !== _p$.i && (_p$.i = setProp(_el$45, "style", _v$36, _p$.i));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0,
								o: void 0,
								i: void 0
							});
							return _el$41;
						})()
					},
					{
						title: "zIndex",
						properties: ["zIndex"],
						render: () => (() => {
							var _el$46 = createElement("view"), _el$47 = createElement("view"), _el$48 = createElement("view");
							insertNode(_el$46, _el$47);
							insertNode(_el$46, _el$48);
							setProp(_el$46, "style", {
								display: "flex",
								flexDirection: "row",
								width: 100,
								height: 40
							});
							effect((_p$) => {
								var _v$37 = {
									width: 56,
									height: 32,
									backgroundColor: p.panel3,
									zIndex: 1,
									borderRadius: 6
								}, _v$38 = {
									width: 56,
									height: 32,
									backgroundColor: p.accent,
									zIndex: 2,
									marginLeft: -24,
									borderRadius: 6
								};
								_v$37 !== _p$.e && (_p$.e = setProp(_el$47, "style", _v$37, _p$.e));
								_v$38 !== _p$.t && (_p$.t = setProp(_el$48, "style", _v$38, _p$.t));
								return _p$;
							}, {
								e: void 0,
								t: void 0
							});
							return _el$46;
						})()
					},
					{
						title: "gridTemplateColumns",
						properties: ["gridTemplateColumns"],
						render: () => (() => {
							var _el$49 = createElement("view"), _el$50 = createElement("view"), _el$51 = createElement("view");
							insertNode(_el$49, _el$50);
							insertNode(_el$49, _el$51);
							effect((_p$) => {
								var _v$39 = {
									display: "grid",
									gridTemplateColumns: ["1fr", "1fr"],
									gap: 6,
									width: 140,
									backgroundColor: p.panel2,
									padding: 6,
									borderRadius: 8
								}, _v$40 = {
									height: 24,
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$41 = {
									height: 24,
									backgroundColor: p.blue,
									borderRadius: 4
								};
								_v$39 !== _p$.e && (_p$.e = setProp(_el$49, "style", _v$39, _p$.e));
								_v$40 !== _p$.t && (_p$.t = setProp(_el$50, "style", _v$40, _p$.t));
								_v$41 !== _p$.a && (_p$.a = setProp(_el$51, "style", _v$41, _p$.a));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0
							});
							return _el$49;
						})()
					},
					{
						title: "gridTemplateRows",
						properties: ["gridTemplateRows"],
						render: () => (() => {
							var _el$52 = createElement("view"), _el$53 = createElement("view"), _el$54 = createElement("view");
							insertNode(_el$52, _el$53);
							insertNode(_el$52, _el$54);
							effect((_p$) => {
								var _v$42 = {
									display: "grid",
									gridTemplateRows: ["1fr", "1fr"],
									gap: 6,
									width: 100,
									height: 72,
									backgroundColor: p.panel2,
									padding: 6,
									borderRadius: 8
								}, _v$43 = {
									backgroundColor: p.accent,
									borderRadius: 4
								}, _v$44 = {
									backgroundColor: p.blue,
									borderRadius: 4
								};
								_v$42 !== _p$.e && (_p$.e = setProp(_el$52, "style", _v$42, _p$.e));
								_v$43 !== _p$.t && (_p$.t = setProp(_el$53, "style", _v$43, _p$.t));
								_v$44 !== _p$.a && (_p$.a = setProp(_el$54, "style", _v$44, _p$.a));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0
							});
							return _el$52;
						})()
					}
				]
			},
			{
				title: "Position & Overflow",
				accent: p.success,
				cards: [{
					title: "position / top / left / right / bottom",
					properties: [
						"position",
						"top",
						"left",
						"right",
						"bottom"
					],
					note: "absolute children pinned to corners",
					render: () => (() => {
						var _el$55 = createElement("view"), _el$56 = createElement("view"), _el$57 = createElement("view");
						insertNode(_el$55, _el$56);
						insertNode(_el$55, _el$57);
						effect((_p$) => {
							var _v$45 = {
								position: "relative",
								width: 160,
								height: 80,
								backgroundColor: p.panel2,
								borderRadius: 8,
								borderWidth: 1,
								borderColor: p.line
							}, _v$46 = {
								position: "absolute",
								top: 8,
								left: 8,
								width: 28,
								height: 28,
								backgroundColor: p.accent,
								borderRadius: 6
							}, _v$47 = {
								position: "absolute",
								right: 8,
								bottom: 8,
								width: 28,
								height: 28,
								backgroundColor: p.accent2,
								borderRadius: 6
							};
							_v$45 !== _p$.e && (_p$.e = setProp(_el$55, "style", _v$45, _p$.e));
							_v$46 !== _p$.t && (_p$.t = setProp(_el$56, "style", _v$46, _p$.t));
							_v$47 !== _p$.a && (_p$.a = setProp(_el$57, "style", _v$47, _p$.a));
							return _p$;
						}, {
							e: void 0,
							t: void 0,
							a: void 0
						});
						return _el$55;
					})()
				}, {
					title: "overflow",
					properties: ["overflow"],
					note: "hidden clips the oversized child",
					render: () => (() => {
						var _el$58 = createElement("view"), _el$59 = createElement("view");
						insertNode(_el$58, _el$59);
						effect((_p$) => {
							var _v$48 = {
								width: 96,
								height: 56,
								overflow: "hidden",
								backgroundColor: p.panel2,
								borderRadius: 8,
								borderWidth: 1,
								borderColor: p.line
							}, _v$49 = {
								width: 160,
								height: 100,
								backgroundColor: p.accent,
								borderRadius: 6
							};
							_v$48 !== _p$.e && (_p$.e = setProp(_el$58, "style", _v$48, _p$.e));
							_v$49 !== _p$.t && (_p$.t = setProp(_el$59, "style", _v$49, _p$.t));
							return _p$;
						}, {
							e: void 0,
							t: void 0
						});
						return _el$58;
					})()
				}]
			},
			{
				title: "Text",
				accent: p.blue,
				cards: [
					{
						title: "fontSize",
						properties: ["fontSize"],
						render: () => (() => {
							var _el$60 = createElement("text");
							insertNode(_el$60, createTextNode(`Sample`));
							effect((_$p) => setProp(_el$60, "style", {
								fontSize: 22,
								color: p.text
							}, _$p));
							return _el$60;
						})()
					},
					{
						title: "fontFamily",
						properties: ["fontFamily"],
						render: () => (() => {
							var _el$62 = createElement("text");
							insertNode(_el$62, createTextNode(`Sample`));
							effect((_$p) => setProp(_el$62, "style", {
								fontFamily: "Georgia, serif",
								color: p.text
							}, _$p));
							return _el$62;
						})()
					},
					{
						title: "fontWeight",
						properties: ["fontWeight"],
						render: () => (() => {
							var _el$64 = createElement("view"), _el$65 = createElement("text"), _el$67 = createElement("text"), _el$69 = createElement("text");
							insertNode(_el$64, _el$65);
							insertNode(_el$64, _el$67);
							insertNode(_el$64, _el$69);
							setProp(_el$64, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 4
							});
							insertNode(_el$65, createTextNode(`Regular 400`));
							insertNode(_el$67, createTextNode(`Semibold 600`));
							insertNode(_el$69, createTextNode(`Bold 700`));
							effect((_p$) => {
								var _v$50 = {
									fontWeight: 400,
									color: p.text
								}, _v$51 = {
									fontWeight: 600,
									color: p.text
								}, _v$52 = {
									fontWeight: 700,
									color: p.text
								};
								_v$50 !== _p$.e && (_p$.e = setProp(_el$65, "style", _v$50, _p$.e));
								_v$51 !== _p$.t && (_p$.t = setProp(_el$67, "style", _v$51, _p$.t));
								_v$52 !== _p$.a && (_p$.a = setProp(_el$69, "style", _v$52, _p$.a));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0
							});
							return _el$64;
						})()
					},
					{
						title: "fontStyle",
						properties: ["fontStyle"],
						render: () => (() => {
							var _el$71 = createElement("view"), _el$72 = createElement("text"), _el$74 = createElement("text");
							insertNode(_el$71, _el$72);
							insertNode(_el$71, _el$74);
							setProp(_el$71, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 4
							});
							insertNode(_el$72, createTextNode(`Upright`));
							insertNode(_el$74, createTextNode(`Italic (synth)`));
							effect((_p$) => {
								var _v$53 = {
									fontStyle: "normal",
									color: p.text
								}, _v$54 = {
									fontStyle: "italic",
									color: p.text
								};
								_v$53 !== _p$.e && (_p$.e = setProp(_el$72, "style", _v$53, _p$.e));
								_v$54 !== _p$.t && (_p$.t = setProp(_el$74, "style", _v$54, _p$.t));
								return _p$;
							}, {
								e: void 0,
								t: void 0
							});
							return _el$71;
						})()
					},
					{
						title: "textDecoration",
						properties: ["textDecoration"],
						render: () => (() => {
							var _el$76 = createElement("text");
							insertNode(_el$76, createTextNode(`Sample`));
							effect((_$p) => setProp(_el$76, "style", {
								textDecoration: "underline",
								color: p.text
							}, _$p));
							return _el$76;
						})()
					},
					{
						title: "color",
						properties: ["color"],
						render: () => (() => {
							var _el$78 = createElement("text");
							insertNode(_el$78, createTextNode(`Sample`));
							effect((_$p) => setProp(_el$78, "style", { color: p.accent }, _$p));
							return _el$78;
						})()
					},
					{
						title: "maxLines / textOverflow",
						properties: ["maxLines", "textOverflow"],
						note: "clamp to 2 lines with ellipsis",
						render: () => (() => {
							var _el$80 = createElement("view"), _el$81 = createElement("text");
							insertNode(_el$80, _el$81);
							setProp(_el$80, "style", { width: 168 });
							insertNode(_el$81, createTextNode(`This caption runs long on purpose so the renderer clamps it to two lines and trails an ellipsis.`));
							effect((_$p) => setProp(_el$81, "style", {
								color: p.text,
								fontSize: 13,
								maxLines: 2,
								textOverflow: "ellipsis"
							}, _$p));
							return _el$80;
						})()
					},
					{
						title: "defaultColor / defaultFontFamily / defaultFontSize / defaultFontWeight",
						properties: [
							"defaultColor",
							"defaultFontFamily",
							"defaultFontSize",
							"defaultFontWeight"
						],
						note: "inherited text defaults",
						render: () => (() => {
							var _el$83 = createElement("view"), _el$84 = createElement("text"), _el$86 = createElement("text");
							insertNode(_el$83, _el$84);
							insertNode(_el$83, _el$86);
							insertNode(_el$84, createTextNode(`Inherited text styles`));
							insertNode(_el$86, createTextNode(`Second line inherits defaults`));
							effect((_$p) => setProp(_el$83, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 6,
								padding: 10,
								backgroundColor: p.panel2,
								borderWidth: 1,
								borderColor: p.line,
								borderRadius: 8,
								defaultColor: p.accent2,
								defaultFontFamily: "Georgia, serif",
								defaultFontSize: 18,
								defaultFontWeight: 700
							}, _$p));
							return _el$83;
						})()
					}
				]
			},
			{
				title: "Motion",
				accent: p.accent,
				cards: [{
					title: "transitionDuration / transitionTiming",
					properties: ["transitionDuration", "transitionTiming"],
					note: "hover to ease the color over 250ms",
					render: () => (() => {
						var _el$88 = createElement("button");
						insertNode(_el$88, createTextNode(`Hover to ease`));
						effect((_$p) => setProp(_el$88, "style", {
							height: 40,
							paddingLeft: 16,
							paddingRight: 16,
							display: "flex",
							alignItems: "center",
							justifyContent: "center",
							backgroundColor: p.panel2,
							defaultColor: p.text,
							borderRadius: 10,
							borderWidth: 1,
							borderColor: p.line,
							transitionDuration: 250,
							transitionTiming: "ease-out",
							":hover": {
								backgroundColor: p.accent,
								defaultColor: p.black,
								borderColor: p.accent
							}
						}, _$p));
						return _el$88;
					})()
				}]
			},
			{
				title: "Interaction & Elements",
				accent: p.accent2,
				cards: [
					{
						title: "cursor",
						properties: ["cursor"],
						note: "hover each tile — the pointer changes and the tile lights up",
						render: () => (() => {
							var _el$90 = createElement("view");
							setProp(_el$90, "style", {
								display: "flex",
								flexWrap: "wrap",
								gap: 6,
								width: 168
							});
							insert(_el$90, () => [
								"pointer",
								"grab",
								"text",
								"not-allowed"
							].map((kind) => (() => {
								var _el$91 = createElement("view"), _el$92 = createElement("text");
								insertNode(_el$91, _el$92);
								setProp(_el$92, "style", { fontSize: 11 });
								insert(_el$92, kind);
								effect((_$p) => setProp(_el$91, "style", {
									width: 78,
									height: 30,
									display: "flex",
									alignItems: "center",
									justifyContent: "center",
									cursor: kind,
									backgroundColor: p.panel2,
									defaultColor: p.text,
									borderRadius: 8,
									borderWidth: 1,
									borderColor: p.line,
									transitionDuration: 150,
									transitionTiming: "ease-out",
									":hover": {
										backgroundColor: p.accent,
										defaultColor: p.black,
										borderColor: p.accent
									}
								}, _$p));
								return _el$91;
							})()));
							return _el$90;
						})()
					},
					{
						title: ":hover",
						properties: [],
						render: () => (() => {
							var _el$93 = createElement("button");
							insertNode(_el$93, createTextNode(`Hover me`));
							effect((_$p) => setProp(_el$93, "style", {
								height: 36,
								paddingLeft: 14,
								paddingRight: 14,
								backgroundColor: p.panel2,
								defaultColor: p.text,
								borderRadius: 10,
								borderWidth: 1,
								borderColor: p.line,
								":hover": {
									backgroundColor: p.accent,
									defaultColor: p.black,
									borderColor: p.accent
								}
							}, _$p));
							return _el$93;
						})()
					},
					{
						title: ":active",
						properties: [],
						render: () => (() => {
							var _el$95 = createElement("button");
							insertNode(_el$95, createTextNode(`Press me`));
							effect((_$p) => setProp(_el$95, "style", {
								height: 36,
								paddingLeft: 14,
								paddingRight: 14,
								backgroundColor: p.panel2,
								defaultColor: p.text,
								borderRadius: 10,
								borderWidth: 1,
								borderColor: p.line,
								":active": {
									backgroundColor: p.accent2,
									defaultColor: p.black,
									borderColor: p.accent2
								}
							}, _$p));
							return _el$95;
						})()
					},
					{
						title: ":focus",
						properties: [],
						render: () => (() => {
							var _el$97 = createElement("text-input");
							setProp(_el$97, "value", "Focus me");
							effect((_$p) => setProp(_el$97, "style", inputStyle(p), _$p));
							return _el$97;
						})()
					},
					{
						title: "scroll-view",
						properties: [],
						render: () => (() => {
							var _el$98 = createElement("scroll-view"), _el$99 = createElement("view");
							insertNode(_el$98, _el$99);
							setProp(_el$99, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 6
							});
							insert(_el$99, () => [
								1,
								2,
								3,
								4,
								5,
								6
							].map((n) => (() => {
								var _el$100 = createElement("text");
								insert(_el$100, `Line ${n}`);
								effect((_$p) => setProp(_el$100, "style", {
									color: p.text,
									fontSize: 12
								}, _$p));
								return _el$100;
							})()));
							effect((_$p) => setProp(_el$98, "style", {
								width: 168,
								height: 72,
								backgroundColor: p.panel2,
								borderWidth: 1,
								borderColor: p.line,
								borderRadius: 8,
								padding: 8
							}, _$p));
							return _el$98;
						})()
					},
					{
						title: "nested scroll (chaining)",
						properties: [],
						render: () => (() => {
							var _el$101 = createElement("scroll-view"), _el$102 = createElement("view"), _el$103 = createElement("text"), _el$105 = createElement("scroll-view"), _el$106 = createElement("view"), _el$107 = createElement("view");
							insertNode(_el$101, _el$102);
							insertNode(_el$102, _el$103);
							insertNode(_el$102, _el$105);
							insertNode(_el$102, _el$107);
							setProp(_el$102, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 8
							});
							insertNode(_el$103, createTextNode(`Outer — scroll past inner edge`));
							insertNode(_el$105, _el$106);
							setProp(_el$106, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 4
							});
							insert(_el$106, () => [
								"A",
								"B",
								"C",
								"D",
								"E"
							].map((c) => (() => {
								var _el$108 = createElement("text");
								insert(_el$108, `Inner ${c}`);
								effect((_$p) => setProp(_el$108, "style", {
									color: p.text,
									fontSize: 11
								}, _$p));
								return _el$108;
							})()));
							setProp(_el$107, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 4
							});
							insert(_el$107, () => [
								1,
								2,
								3,
								4
							].map((n) => (() => {
								var _el$109 = createElement("text");
								insert(_el$109, `Outer tail ${n}`);
								effect((_$p) => setProp(_el$109, "style", {
									color: p.text,
									fontSize: 11
								}, _$p));
								return _el$109;
							})()));
							effect((_p$) => {
								var _v$55 = {
									width: 180,
									height: 120,
									backgroundColor: p.panel,
									borderWidth: 1,
									borderColor: p.accent,
									borderRadius: 8,
									padding: 6
								}, _v$56 = {
									color: p.muted,
									fontSize: 11
								}, _v$57 = {
									width: 160,
									height: 64,
									backgroundColor: p.panel2,
									borderWidth: 1,
									borderColor: p.line,
									borderRadius: 6,
									padding: 6
								};
								_v$55 !== _p$.e && (_p$.e = setProp(_el$101, "style", _v$55, _p$.e));
								_v$56 !== _p$.t && (_p$.t = setProp(_el$103, "style", _v$56, _p$.t));
								_v$57 !== _p$.a && (_p$.a = setProp(_el$105, "style", _v$57, _p$.a));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0
							});
							return _el$101;
						})()
					},
					{
						title: "text-input",
						properties: [],
						render: () => (() => {
							var _el$110 = createElement("text-input");
							setProp(_el$110, "placeholder", "Type here");
							setProp(_el$110, "value", "");
							effect((_$p) => setProp(_el$110, "style", inputStyle(p), _$p));
							return _el$110;
						})()
					},
					{
						title: "button",
						properties: [],
						render: () => (() => {
							var _el$111 = createElement("button");
							insertNode(_el$111, createTextNode(`Click`));
							effect((_$p) => setProp(_el$111, "style", {
								height: 36,
								paddingLeft: 14,
								paddingRight: 14,
								backgroundColor: p.blue,
								defaultColor: p.black,
								borderRadius: 10,
								borderWidth: 1,
								borderColor: p.blue
							}, _$p));
							return _el$111;
						})()
					},
					{
						title: "user-select",
						properties: [],
						note: "view/text 既定選択可・user-select:none で除外",
						render: () => (() => {
							var _el$113 = createElement("view"), _el$114 = createElement("view"), _el$115 = createElement("text"), _el$117 = createElement("view"), _el$118 = createElement("text");
							insertNode(_el$113, _el$114);
							insertNode(_el$113, _el$117);
							setProp(_el$113, "style", {
								display: "flex",
								flexDirection: "column",
								gap: 6
							});
							insertNode(_el$114, _el$115);
							insertNode(_el$115, createTextNode(`既定で選択できる（宣言なし）`));
							insertNode(_el$117, _el$118);
							setProp(_el$117, "user-select", "none");
							insertNode(_el$118, createTextNode(`user-select: none で選択不可`));
							effect((_p$) => {
								var _v$58 = {
									padding: 8,
									backgroundColor: p.panel2,
									borderRadius: 8,
									borderWidth: 1,
									borderColor: p.line
								}, _v$59 = {
									color: p.text,
									fontSize: 12
								}, _v$60 = {
									padding: 8,
									backgroundColor: p.panel2,
									borderRadius: 8,
									borderWidth: 1,
									borderColor: p.line
								}, _v$61 = {
									color: p.muted,
									fontSize: 12
								};
								_v$58 !== _p$.e && (_p$.e = setProp(_el$114, "style", _v$58, _p$.e));
								_v$59 !== _p$.t && (_p$.t = setProp(_el$115, "style", _v$59, _p$.t));
								_v$60 !== _p$.a && (_p$.a = setProp(_el$117, "style", _v$60, _p$.a));
								_v$61 !== _p$.o && (_p$.o = setProp(_el$118, "style", _v$61, _p$.o));
								return _p$;
							}, {
								e: void 0,
								t: void 0,
								a: void 0,
								o: void 0
							});
							return _el$113;
						})()
					}
				]
			},
			{
				title: "Responsive",
				accent: p.success,
				cards: [{
					title: "@media / styleVariants",
					properties: [],
					note: "ウィンドウ幅を変えると一致する帯だけ点灯。DOM では本物の @media ルール（DevTools の <style data-tsubame-variant>）。",
					render: () => createComponent(MediaTiles, { colors: p })
				}]
			}
		];
	}
	var ROADMAP = [
		["transform", "2D/3D transforms (translate, scale, rotate)"],
		["textAlign", "Horizontal text alignment"],
		["lineHeight", "Line box height for text"],
		["letterSpacing", "Tracking between glyphs"],
		["outline", "Focus ring outside border box"]
	];
	/** Catalog patchKeys with a live POP card, derived from the section descriptors. */
	var GALLERY_LIVE_PROPERTIES = buildSections(palette(DEFAULT_THEME, DEFAULT_ACCENT)).flatMap((section) => section.cards).flatMap((card) => card.properties);
	ROADMAP.map(([name]) => name);
	//#endregion
	//#region src/gallery/SectionView.tsx
	function PopCard(props) {
		return (() => {
			var _el$ = createElement("view"), _el$2 = createElement("view"), _el$3 = createElement("view"), _el$4 = createElement("text"), _el$5 = createElement("view");
			insertNode(_el$, _el$2);
			insertNode(_el$, _el$5);
			insertNode(_el$2, _el$3);
			insertNode(_el$2, _el$4);
			setProp(_el$2, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				gap: 8
			});
			insert(_el$4, () => props.title);
			insert(_el$5, () => props.children);
			insert(_el$, (() => {
				var _c$ = memo(() => !!props.note);
				return () => _c$() ? (() => {
					var _el$6 = createElement("text");
					insert(_el$6, () => props.note);
					effect((_$p) => setProp(_el$6, "style", {
						color: props.colors.quiet,
						fontSize: 11
					}, _$p));
					return _el$6;
				})() : null;
			})(), null);
			effect((_p$) => {
				var _v$ = {
					display: "flex",
					flexDirection: "column",
					gap: 12,
					minWidth: 200,
					maxWidth: 268,
					padding: 16,
					backgroundColor: props.colors.panel,
					borderRadius: 16,
					borderWidth: 1,
					borderColor: props.colors.line
				}, _v$2 = {
					width: 10,
					height: 10,
					borderRadius: 6,
					backgroundColor: props.accent
				}, _v$3 = {
					color: props.accent,
					fontSize: 13,
					fontWeight: 600
				}, _v$4 = {
					display: "flex",
					flexDirection: "column",
					gap: 8,
					alignItems: "flex-start",
					padding: 14,
					backgroundColor: props.colors.bg,
					borderRadius: 12,
					borderWidth: 1,
					borderColor: props.colors.line
				};
				_v$ !== _p$.e && (_p$.e = setProp(_el$, "style", _v$, _p$.e));
				_v$2 !== _p$.t && (_p$.t = setProp(_el$3, "style", _v$2, _p$.t));
				_v$3 !== _p$.a && (_p$.a = setProp(_el$4, "style", _v$3, _p$.a));
				_v$4 !== _p$.o && (_p$.o = setProp(_el$5, "style", _v$4, _p$.o));
				return _p$;
			}, {
				e: void 0,
				t: void 0,
				a: void 0,
				o: void 0
			});
			return _el$;
		})();
	}
	function SectionView(props) {
		return (() => {
			var _el$7 = createElement("view"), _el$8 = createElement("view"), _el$9 = createElement("view"), _el$0 = createElement("text"), _el$1 = createElement("view");
			insertNode(_el$7, _el$8);
			insertNode(_el$7, _el$1);
			setProp(_el$7, "style", {
				display: "flex",
				flexDirection: "column",
				gap: 14
			});
			insertNode(_el$8, _el$9);
			insertNode(_el$8, _el$0);
			setProp(_el$8, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				gap: 10
			});
			insert(_el$0, () => props.section.title);
			setProp(_el$1, "style", {
				display: "flex",
				flexWrap: "wrap",
				gap: 14
			});
			insert(_el$1, () => props.section.cards.map((card) => createComponent(PopCard, {
				get colors() {
					return props.colors;
				},
				get title() {
					return card.title;
				},
				get accent() {
					return props.section.accent;
				},
				get note() {
					return card.note;
				},
				get children() {
					return card.render();
				}
			})));
			effect((_p$) => {
				var _v$5 = {
					width: 4,
					height: 22,
					borderRadius: 3,
					backgroundColor: props.section.accent
				}, _v$6 = {
					color: props.colors.ink,
					fontSize: 18,
					fontWeight: 600
				};
				_v$5 !== _p$.e && (_p$.e = setProp(_el$9, "style", _v$5, _p$.e));
				_v$6 !== _p$.t && (_p$.t = setProp(_el$0, "style", _v$6, _p$.t));
				return _p$;
			}, {
				e: void 0,
				t: void 0
			});
			return _el$7;
		})();
	}
	function RoadmapSection(props) {
		return (() => {
			var _el$10 = createElement("view"), _el$11 = createElement("view"), _el$12 = createElement("view"), _el$13 = createElement("text"), _el$15 = createElement("view"), _el$16 = createElement("text");
			insertNode(_el$10, _el$11);
			insertNode(_el$10, _el$15);
			setProp(_el$10, "style", {
				display: "flex",
				flexDirection: "column",
				gap: 14
			});
			insertNode(_el$11, _el$12);
			insertNode(_el$11, _el$13);
			setProp(_el$11, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				gap: 10
			});
			insertNode(_el$13, createTextNode(`Roadmap / 未実装`));
			insertNode(_el$15, _el$16);
			insertNode(_el$16, createTextNode(`Future CSS candidates not yet in style_tags.json — shown as static reference only.`));
			insert(_el$15, () => ROADMAP.map(([name, description]) => (() => {
				var _el$18 = createElement("view"), _el$19 = createElement("text"), _el$20 = createElement("text");
				insertNode(_el$18, _el$19);
				insertNode(_el$18, _el$20);
				setProp(_el$18, "style", {
					display: "flex",
					flexDirection: "column",
					gap: 2
				});
				insert(_el$19, name);
				insert(_el$20, description);
				effect((_p$) => {
					var _v$1 = {
						color: props.colors.accent2,
						fontSize: 14
					}, _v$10 = {
						color: props.colors.quiet,
						fontSize: 12
					};
					_v$1 !== _p$.e && (_p$.e = setProp(_el$19, "style", _v$1, _p$.e));
					_v$10 !== _p$.t && (_p$.t = setProp(_el$20, "style", _v$10, _p$.t));
					return _p$;
				}, {
					e: void 0,
					t: void 0
				});
				return _el$18;
			})()), null);
			effect((_p$) => {
				var _v$7 = {
					width: 4,
					height: 22,
					borderRadius: 3,
					backgroundColor: props.colors.quiet
				}, _v$8 = {
					color: props.colors.ink,
					fontSize: 18,
					fontWeight: 600
				}, _v$9 = {
					display: "flex",
					flexDirection: "column",
					gap: 10,
					width: "100%",
					padding: 16,
					backgroundColor: props.colors.panel,
					borderRadius: 16,
					borderWidth: 1,
					borderColor: props.colors.line
				}, _v$0 = {
					color: props.colors.muted,
					fontSize: 13
				};
				_v$7 !== _p$.e && (_p$.e = setProp(_el$12, "style", _v$7, _p$.e));
				_v$8 !== _p$.t && (_p$.t = setProp(_el$13, "style", _v$8, _p$.t));
				_v$9 !== _p$.a && (_p$.a = setProp(_el$15, "style", _v$9, _p$.a));
				_v$0 !== _p$.o && (_p$.o = setProp(_el$16, "style", _v$0, _p$.o));
				return _p$;
			}, {
				e: void 0,
				t: void 0,
				a: void 0,
				o: void 0
			});
			return _el$10;
		})();
	}
	//#endregion
	//#region src/CssGallery.tsx
	function CssGallery(props) {
		const sections = buildSections(props.colors);
		return (() => {
			var _el$ = createElement("scroll-view"), _el$2 = createElement("view"), _el$3 = createElement("text"), _el$5 = createElement("text");
			insertNode(_el$, _el$2);
			insertNode(_el$2, _el$3);
			insertNode(_el$2, _el$5);
			setProp(_el$2, "style", {
				display: "flex",
				flexDirection: "column",
				gap: 6
			});
			insertNode(_el$3, createTextNode(`CSS Gallery`));
			insert(_el$5, () => `${GALLERY_LIVE_PROPERTIES.length} HayateStyle properties — each POP card renders the property live, in DOM and Canvas.`);
			insert(_el$, () => sections.map((section) => createComponent(SectionView, {
				get colors() {
					return props.colors;
				},
				section
			})), null);
			insert(_el$, createComponent(RoadmapSection, { get colors() {
				return props.colors;
			} }), null);
			effect((_p$) => {
				var _v$ = {
					width: "100%",
					height: "100%",
					display: "flex",
					flexDirection: "column",
					gap: 28,
					paddingTop: 18,
					paddingLeft: 28,
					paddingRight: 28,
					paddingBottom: 28,
					backgroundColor: props.colors.bg
				}, _v$2 = {
					color: props.colors.ink,
					fontSize: 24,
					fontWeight: 700
				}, _v$3 = {
					color: props.colors.muted,
					fontSize: 13
				};
				_v$ !== _p$.e && (_p$.e = setProp(_el$, "style", _v$, _p$.e));
				_v$2 !== _p$.t && (_p$.t = setProp(_el$3, "style", _v$2, _p$.t));
				_v$3 !== _p$.a && (_p$.a = setProp(_el$5, "style", _v$3, _p$.a));
				return _p$;
			}, {
				e: void 0,
				t: void 0,
				a: void 0
			});
			return _el$;
		})();
	}
	//#endregion
	//#region src/todo-model.ts
	/** 表示フィルタの正本。UI のチップ順もこの順に従う。 */
	var FILTER_VALUES = [
		"all",
		"active",
		"done"
	];
	/** 並び順の正本。UI のチップ順もこの順に従う。 */
	var SORT_VALUES = [
		"manual",
		"name",
		"prio"
	];
	/** 優先度の正本。追加フォームのセグメント順（高→低）もこの順に従う。 */
	var PRIORITY_VALUES = [
		3,
		2,
		1
	];
	/** 新規タスクを先頭に挿入する（未完了で作成）。空文字・空白のみは無視。 */
	function add(todos, draft) {
		const text = draft.text.trim();
		if (!text) return [...todos];
		return [{
			id: draft.id,
			text,
			prio: draft.prio,
			done: false
		}, ...todos];
	}
	/** 指定 id の完了/未完了をトグルする。 */
	function toggleDone(todos, id) {
		return todos.map((todo) => todo.id === id ? {
			...todo,
			done: !todo.done
		} : todo);
	}
	/** 指定 id の文言を編集する（trim 後）。空文字は無視して変更しない。 */
	function editText(todos, id, text) {
		const trimmed = text.trim();
		if (!trimmed) return [...todos];
		return todos.map((todo) => todo.id === id ? {
			...todo,
			text: trimmed
		} : todo);
	}
	/** 指定 id のタスクを削除する。 */
	function remove(todos, id) {
		return todos.filter((todo) => todo.id !== id);
	}
	/** 完了済みタスクを一括削除する。 */
	function clearDone(todos) {
		return todos.filter((todo) => !todo.done);
	}
	/** index i と i+1 を入れ替える。範囲外なら変更しない。 */
	function swap(todos, i) {
		if (i < 0 || i + 1 >= todos.length) return [...todos];
		const next = [...todos];
		[next[i], next[i + 1]] = [next[i + 1], next[i]];
		return next;
	}
	/** 指定 id を一つ上へ移動する（手動並べ替え）。 */
	function moveUp(todos, id) {
		return swap(todos, todos.findIndex((todo) => todo.id === id) - 1);
	}
	/** 指定 id を一つ下へ移動する（手動並べ替え）。 */
	function moveDown(todos, id) {
		return swap(todos, todos.findIndex((todo) => todo.id === id));
	}
	/**
	* 手動並べ替え（moveUp/moveDown）が意味を持つ並び順かを返す。
	* name/prio は表示順が導出されるため、上/下ボタンは manual のときだけ有効。
	*/
	function canReorder(sort) {
		return sort === "manual";
	}
	/** 表示フィルタを適用する（all / active=未完了 / done=完了）。 */
	function filterTodos(todos, filter) {
		if (filter === "active") return todos.filter((todo) => !todo.done);
		if (filter === "done") return todos.filter((todo) => todo.done);
		return [...todos];
	}
	/** 並び順を適用する（manual=手動 / name=名前(ja) / prio=優先度降順）。常に新配列を返す。 */
	function sortTodos(todos, sort) {
		const next = [...todos];
		if (sort === "name") return next.sort((a, b) => a.text.localeCompare(b.text, "ja"));
		if (sort === "prio") return next.sort((a, b) => b.prio - a.prio);
		return next;
	}
	/**
	* 単カードのリストに表示する Todo を導出する。
	* フィルタ → ソートの順で適用する（gomi の単カードと同じ可視化規則）。常に新配列。
	*/
	function visibleTodos(todos, filter, sort) {
		return sortTodos(filterTodos(todos, filter), sort);
	}
	/** 完了状況を集計する（残り件数 / 全件 / 完了率%）。 */
	function completion(todos) {
		const total = todos.length;
		const remaining = todos.filter((todo) => !todo.done).length;
		return {
			total,
			remaining,
			percent: total === 0 ? 0 : Math.round((total - remaining) / total * 100)
		};
	}
	/** 永続化が空・破損していたときに使う初期データ。 */
	var SEED = [
		{
			id: 1,
			text: "レイアウトエンジンに flex-wrap を実装",
			prio: 3,
			done: false
		},
		{
			id: 2,
			text: "box-shadow の描画を確認する",
			prio: 2,
			done: true
		},
		{
			id: 3,
			text: "ドラッグで並べ替えできるかテスト",
			prio: 2,
			done: false
		},
		{
			id: 4,
			text: "ダークモードの配色を調整",
			prio: 1,
			done: false
		},
		{
			id: 5,
			text: "sticky ヘッダーの挙動チェック",
			prio: 3,
			done: true
		}
	];
	//#endregion
	//#region src/ui/labels.ts
	/** 優先度の表示ラベル（追加フォーム・行で共有）。 */
	var PRIORITY_LABEL = {
		3: "高",
		2: "中",
		1: "低"
	};
	var FILTER_LABEL = {
		all: "すべて",
		active: "未完了",
		done: "完了済み"
	};
	/** ツールバーのフィルタ chip。モデルの正本 `FILTER_VALUES` から導出する。 */
	var FILTERS = FILTER_VALUES.map((value) => ({
		value,
		label: FILTER_LABEL[value]
	}));
	var SORT_LABEL = {
		manual: "手動",
		name: "名前",
		prio: "優先度"
	};
	/** ツールバーのソート chip。モデルの正本 `SORT_VALUES` から導出する。 */
	var SORTS = SORT_VALUES.map((value) => ({
		value,
		label: SORT_LABEL[value]
	}));
	/** 追加フォームの優先度セグメント。モデルの正本 `PRIORITY_VALUES` から導出する。 */
	var PRIORITIES = [...PRIORITY_VALUES];
	/** インライン編集の確定/取消キーを判定する（Enter=確定 / Escape=取消 / それ以外=無視）。 */
	function editKeyAction(key) {
		if (key === "Enter") return "commit";
		if (key === "Escape") return "cancel";
		return "none";
	}
	//#endregion
	//#region src/ui/styles.ts
	/**
	* 共通イージング（ADR-0067 / Transition）。全インタラクティブ要素に同じ
	* 補間を載せ、hover / active / focus の状態切替を一瞬ではなく滑らかにする。
	* 補間対象は連続値（color / border / box-shadow / opacity / radius）のみ。
	*/
	var EASE = {
		transitionDuration: 160,
		transitionTiming: "ease-out"
	};
	/** アクセント色のグロー影。主要 CTA を POP に浮かせる（ADR-0095）。 */
	var glow = (color, strong = false) => [{
		offsetX: 0,
		offsetY: strong ? 8 : 5,
		blur: strong ? 22 : 16,
		spread: -4,
		color,
		inset: false
	}];
	/** 優先度→色。danger(高) / accent2(中) / blue(低) に対応する。 */
	function priorityTone(p, prio) {
		if (prio === 3) return p.danger;
		if (prio === 2) return p.accent2;
		return p.blue;
	}
	//#endregion
	//#region src/components/AddForm.tsx
	function AddForm(props) {
		const seg = (active, tone) => ({
			height: 38,
			minWidth: 40,
			display: "flex",
			alignItems: "center",
			justifyContent: "center",
			backgroundColor: active ? tone : props.colors.panel2,
			defaultColor: active ? props.colors.black : props.colors.muted,
			borderRadius: 9,
			borderWidth: 1,
			borderStyle: "solid",
			borderColor: active ? tone : props.colors.line,
			defaultFontSize: 13,
			boxShadow: active ? glow(`${tone}55`) : [],
			...EASE,
			":hover": {
				backgroundColor: active ? tone : props.colors.panel3,
				borderColor: active ? tone : props.colors.line
			}
		});
		return (() => {
			var _el$ = createElement("view"), _el$2 = createElement("view"), _el$3 = createElement("text-input"), _el$4 = createElement("view"), _el$5 = createElement("button");
			insertNode(_el$, _el$2);
			insertNode(_el$, _el$4);
			insertNode(_el$, _el$5);
			setProp(_el$, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				flexWrap: "wrap",
				gap: 8
			});
			insertNode(_el$2, _el$3);
			setProp(_el$2, "style", {
				flexGrow: 1,
				minWidth: 180
			});
			setProp(_el$3, "placeholder", "新しいタスクを入力…");
			setProp(_el$3, "onInput", (event) => props.onInput(event.value ?? ""));
			setProp(_el$3, "onKeyDown", (event) => {
				if (event.key === "Enter") props.onAdd();
			});
			setProp(_el$4, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				gap: 4
			});
			insert(_el$4, () => PRIORITIES.map((prio) => (() => {
				var _el$7 = createElement("button");
				setProp(_el$7, "onClick", () => props.onPrio(prio));
				insert(_el$7, () => PRIORITY_LABEL[prio]);
				effect((_$p) => setProp(_el$7, "style", seg(props.prio === prio, priorityTone(props.colors, prio)), _$p));
				return _el$7;
			})()));
			insertNode(_el$5, createTextNode(`追加`));
			effect((_p$) => {
				var _v$ = props.draft, _v$2 = inputStyle(props.colors), _v$3 = {
					height: 38,
					paddingLeft: 18,
					paddingRight: 18,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					backgroundColor: props.colors.accent,
					defaultColor: props.colors.black,
					borderRadius: 9,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: props.colors.accent,
					defaultFontSize: 13,
					boxShadow: glow(`${props.colors.accent}55`),
					...EASE,
					":hover": {
						backgroundColor: props.colors.success,
						borderColor: props.colors.success,
						boxShadow: glow(`${props.colors.success}77`, true)
					}
				}, _v$4 = props.onAdd;
				_v$ !== _p$.e && (_p$.e = setProp(_el$3, "value", _v$, _p$.e));
				_v$2 !== _p$.t && (_p$.t = setProp(_el$3, "style", _v$2, _p$.t));
				_v$3 !== _p$.a && (_p$.a = setProp(_el$5, "style", _v$3, _p$.a));
				_v$4 !== _p$.o && (_p$.o = setProp(_el$5, "onClick", _v$4, _p$.o));
				return _p$;
			}, {
				e: void 0,
				t: void 0,
				a: void 0,
				o: void 0
			});
			return _el$;
		})();
	}
	//#endregion
	//#region src/components/AppBar.tsx
	/** 水平スペーサ（幅 w の不可視 view）。AppBar の左右インセット調整に使う。 */
	var SpX = (w) => (() => {
		var _el$ = createElement("view");
		setProp(_el$, "style", {
			width: w,
			height: 1
		});
		return _el$;
	})();
	/** 検出済みレンダラの表示名（DOM ならそのまま、Canvas はバックエンド名）。 */
	function rendererBadge(detected) {
		if (detected.mode === "DOM") return "DOM";
		return detected.backend ?? "Canvas";
	}
	function AppBar(props) {
		const tab = (active) => ({
			height: 34,
			paddingLeft: 16,
			paddingRight: 16,
			display: "flex",
			alignItems: "center",
			justifyContent: "center",
			backgroundColor: active ? props.colors.accent : props.colors.panel,
			defaultColor: active ? props.colors.black : props.colors.text,
			borderRadius: 10,
			borderWidth: 1,
			borderStyle: "solid",
			borderColor: active ? props.colors.accent : props.colors.line,
			defaultFontSize: 13,
			boxShadow: active ? glow(`${props.colors.accent}44`) : [],
			...EASE,
			":hover": {
				backgroundColor: active ? props.colors.accent : props.colors.panel3,
				borderColor: active ? props.colors.accent : props.colors.line
			}
		});
		const swatch = (key) => {
			const selected = props.accent === key;
			return {
				width: 22,
				height: 22,
				backgroundColor: accentColor(props.theme, key),
				borderRadius: 999,
				borderWidth: selected ? 3 : 1,
				borderStyle: "solid",
				borderColor: selected ? props.colors.ink : props.colors.line,
				boxShadow: selected ? glow(`${accentColor(props.theme, key)}66`) : [],
				...EASE,
				":hover": { borderColor: props.colors.ink }
			};
		};
		return (() => {
			var _el$2 = createElement("view"), _el$3 = createElement("view"), _el$4 = createElement("view"), _el$5 = createElement("text"), _el$7 = createElement("view"), _el$8 = createElement("text"), _el$0 = createElement("text"), _el$10 = createElement("view"), _el$11 = createElement("button"), _el$13 = createElement("button"), _el$15 = createElement("view"), _el$16 = createElement("view"), _el$17 = createElement("button"), _el$18 = createElement("view"), _el$19 = createElement("text"), _el$21 = createElement("view"), _el$22 = createElement("text"), _el$23 = createElement("view"), _el$24 = createElement("text");
			insertNode(_el$2, _el$3);
			insertNode(_el$2, _el$10);
			setProp(_el$2, "styleVariants", [{
				condition: { maxWidth: 719 },
				style: {
					flexDirection: "column",
					flexWrap: "nowrap",
					alignItems: "flex-start"
				}
			}]);
			insertNode(_el$3, _el$4);
			insertNode(_el$3, _el$7);
			setProp(_el$3, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				gap: 12
			});
			insert(_el$3, () => SpX(24), _el$4);
			insertNode(_el$4, _el$5);
			insertNode(_el$5, createTextNode(`TS`));
			insertNode(_el$7, _el$8);
			insertNode(_el$7, _el$0);
			setProp(_el$7, "style", {
				display: "flex",
				flexDirection: "column",
				gap: 2
			});
			insertNode(_el$8, createTextNode(`Tsubame Task Studio`));
			setProp(_el$8, "styleVariants", [{
				condition: { maxWidth: 719 },
				style: { fontSize: 17 }
			}]);
			insertNode(_el$0, createTextNode(`POP TODO + Hayate CSS gallery`));
			setProp(_el$0, "styleVariants", [{
				condition: { maxWidth: 719 },
				style: { display: "none" }
			}]);
			insertNode(_el$10, _el$11);
			insertNode(_el$10, _el$13);
			insertNode(_el$10, _el$15);
			insertNode(_el$10, _el$16);
			insertNode(_el$10, _el$17);
			insertNode(_el$10, _el$18);
			setProp(_el$10, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				flexWrap: "wrap",
				gap: 10
			});
			insertNode(_el$11, createTextNode(`Tasks`));
			setProp(_el$11, "onClick", () => props.setPage("tasks"));
			insertNode(_el$13, createTextNode(`CSS Gallery`));
			setProp(_el$13, "onClick", () => props.setPage("gallery"));
			setProp(_el$16, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				gap: 6
			});
			insert(_el$16, () => ACCENT_KEYS.map((key) => (() => {
				var _el$25 = createElement("button");
				insertNode(_el$25, createTextNode(` `));
				setProp(_el$25, "onClick", () => props.onAccent(key));
				effect((_$p) => setProp(_el$25, "style", swatch(key), _$p));
				return _el$25;
			})()));
			insert(_el$17, () => props.theme === "dark" ? "☀" : "🌙");
			insertNode(_el$18, _el$19);
			insertNode(_el$18, _el$21);
			setProp(_el$18, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				gap: 8
			});
			setProp(_el$18, "styleVariants", [{
				condition: { maxWidth: 719 },
				style: { display: "none" }
			}]);
			insertNode(_el$19, createTextNode(`renderer`));
			insertNode(_el$21, _el$22);
			insertNode(_el$21, _el$23);
			insertNode(_el$21, _el$24);
			insert(_el$21, () => SpX(12), _el$22);
			insert(_el$22, () => rendererBadge(props.detected));
			insert(_el$21, () => SpX(10), _el$23);
			insert(_el$21, () => SpX(10), _el$24);
			insert(_el$24, (() => {
				var _c$ = memo(() => props.detected.source === "query");
				return () => _c$() ? props.detected.renderer : "auto";
			})());
			insert(_el$21, () => SpX(12), null);
			insert(_el$10, () => SpX(100), null);
			effect((_p$) => {
				var _v$ = {
					minHeight: 64,
					display: "flex",
					flexDirection: "row",
					alignItems: "center",
					justifyContent: "space-between",
					flexWrap: "wrap",
					gap: 12,
					paddingTop: 8,
					paddingBottom: 8,
					backgroundColor: props.colors.rail,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: props.colors.line
				}, _v$2 = {
					width: 38,
					height: 38,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					backgroundColor: props.colors.accent,
					borderRadius: 12
				}, _v$3 = {
					fontSize: 18,
					color: props.colors.black
				}, _v$4 = {
					fontSize: 20,
					color: props.colors.ink
				}, _v$5 = {
					fontSize: 12,
					color: props.colors.muted
				}, _v$6 = tab(props.page === "tasks"), _v$7 = tab(props.page === "gallery"), _v$8 = {
					width: 1,
					height: 22,
					backgroundColor: props.colors.line
				}, _v$9 = {
					width: 34,
					height: 34,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					backgroundColor: props.colors.panel,
					defaultColor: props.colors.text,
					borderRadius: 10,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: props.colors.line,
					defaultFontSize: 15,
					...EASE,
					":hover": {
						backgroundColor: props.colors.panel3,
						borderColor: props.colors.line
					}
				}, _v$0 = props.onToggleTheme, _v$1 = {
					color: props.colors.quiet,
					fontSize: 11
				}, _v$10 = {
					height: 28,
					display: "flex",
					flexDirection: "row",
					alignItems: "center",
					backgroundColor: props.colors.panel,
					borderRadius: 10,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: props.colors.line
				}, _v$11 = {
					color: props.colors.accent,
					fontSize: 13
				}, _v$12 = {
					width: 1,
					height: 16,
					backgroundColor: props.colors.line
				}, _v$13 = {
					color: props.colors.muted,
					fontSize: 12
				};
				_v$ !== _p$.e && (_p$.e = setProp(_el$2, "style", _v$, _p$.e));
				_v$2 !== _p$.t && (_p$.t = setProp(_el$4, "style", _v$2, _p$.t));
				_v$3 !== _p$.a && (_p$.a = setProp(_el$5, "style", _v$3, _p$.a));
				_v$4 !== _p$.o && (_p$.o = setProp(_el$8, "style", _v$4, _p$.o));
				_v$5 !== _p$.i && (_p$.i = setProp(_el$0, "style", _v$5, _p$.i));
				_v$6 !== _p$.n && (_p$.n = setProp(_el$11, "style", _v$6, _p$.n));
				_v$7 !== _p$.s && (_p$.s = setProp(_el$13, "style", _v$7, _p$.s));
				_v$8 !== _p$.h && (_p$.h = setProp(_el$15, "style", _v$8, _p$.h));
				_v$9 !== _p$.r && (_p$.r = setProp(_el$17, "style", _v$9, _p$.r));
				_v$0 !== _p$.d && (_p$.d = setProp(_el$17, "onClick", _v$0, _p$.d));
				_v$1 !== _p$.l && (_p$.l = setProp(_el$19, "style", _v$1, _p$.l));
				_v$10 !== _p$.u && (_p$.u = setProp(_el$21, "style", _v$10, _p$.u));
				_v$11 !== _p$.c && (_p$.c = setProp(_el$22, "style", _v$11, _p$.c));
				_v$12 !== _p$.w && (_p$.w = setProp(_el$23, "style", _v$12, _p$.w));
				_v$13 !== _p$.m && (_p$.m = setProp(_el$24, "style", _v$13, _p$.m));
				return _p$;
			}, {
				e: void 0,
				t: void 0,
				a: void 0,
				o: void 0,
				i: void 0,
				n: void 0,
				s: void 0,
				h: void 0,
				r: void 0,
				d: void 0,
				l: void 0,
				u: void 0,
				c: void 0,
				w: void 0,
				m: void 0
			});
			return _el$2;
		})();
	}
	//#endregion
	//#region src/components/TaskCard.tsx
	/**
	* タスクカードを構成する小さなプレゼンテーション部品群。いずれもタスク画面専用で
	* `Palette` のみに依存し、他画面では再利用しないため、1ファイルにまとめる。
	*/
	function Header(props) {
		return (() => {
			var _el$ = createElement("view"), _el$2 = createElement("view"), _el$3 = createElement("text"), _el$5 = createElement("text");
			insertNode(_el$, _el$2);
			setProp(_el$, "style", {
				display: "flex",
				flexDirection: "column",
				gap: 12
			});
			insertNode(_el$2, _el$3);
			insertNode(_el$2, _el$5);
			setProp(_el$2, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				justifyContent: "space-between"
			});
			insertNode(_el$3, createTextNode(`きょうのタスク`));
			insert(_el$5, () => `残り ${props.remaining} 件 / 全 ${props.total} 件`);
			insert(_el$, createComponent(ProgressBar, {
				get colors() {
					return props.colors;
				},
				get percent() {
					return props.percent;
				}
			}), null);
			effect((_p$) => {
				var _v$ = {
					color: props.colors.ink,
					fontSize: 24
				}, _v$2 = {
					color: props.colors.muted,
					fontSize: 13
				};
				_v$ !== _p$.e && (_p$.e = setProp(_el$3, "style", _v$, _p$.e));
				_v$2 !== _p$.t && (_p$.t = setProp(_el$5, "style", _v$2, _p$.t));
				return _p$;
			}, {
				e: void 0,
				t: void 0
			});
			return _el$;
		})();
	}
	function ProgressBar(props) {
		return (() => {
			var _el$6 = createElement("view"), _el$7 = createElement("view");
			insertNode(_el$6, _el$7);
			effect((_p$) => {
				var _v$3 = {
					width: "100%",
					height: 12,
					display: "flex",
					flexDirection: "row",
					alignItems: "center",
					backgroundColor: props.colors.black,
					borderRadius: 8,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: props.colors.line
				}, _v$4 = {
					width: `${props.percent}%`,
					height: 8,
					marginLeft: 2,
					backgroundColor: props.colors.success,
					borderRadius: 6
				};
				_v$3 !== _p$.e && (_p$.e = setProp(_el$6, "style", _v$3, _p$.e));
				_v$4 !== _p$.t && (_p$.t = setProp(_el$7, "style", _v$4, _p$.t));
				return _p$;
			}, {
				e: void 0,
				t: void 0
			});
			return _el$6;
		})();
	}
	/**
	* 読み取り専用テキストの選択ジェスチャデモ（ADR-0108、ADR-0097 を supersede /
	* issue #266・#267・#268・#269）。
	*
	* CSS `user-select` と同型で、view / text は**宣言なしで既定選択可**（opt-out）。
	* 明示 `user-select: none` を置いた subtree だけが選択から除外される。DOM Mode
	* ではブラウザのネイティブ選択に委ね、ドラッグに加えダブルクリックで単語・
	* トリプルクリックで段落、Shift+クリック / Shift+矢印で範囲拡張、Cmd/Ctrl+A で
	* 全選択ができる。Cmd/Ctrl+C で選択テキストが Platform Adapter 経由でクリップ
	* ボードへコピーされる。
	*
	* 末尾のキャプションは `user-select: none` を持つ view に包まれており、本文を
	* 全選択しても選択対象に入らない（opt-out の確認）。
	*/
	function SelectableNote(props) {
		const para = {
			color: props.colors.muted,
			fontSize: 13
		};
		return (() => {
			var _el$8 = createElement("view"), _el$9 = createElement("text"), _el$1 = createElement("text"), _el$11 = createElement("view"), _el$12 = createElement("text");
			insertNode(_el$8, _el$9);
			insertNode(_el$8, _el$1);
			insertNode(_el$8, _el$11);
			insertNode(_el$9, createTextNode(`この段落は宣言なしで選択できます。ダブルクリックで単語、トリプルクリックで段落を選び、Shift+クリックや Shift+矢印で範囲を伸縮、Cmd/Ctrl+A で全選択できます。選択して Cmd/Ctrl+C を押すとクリップボードへコピーされ、別アプリへ貼り付けられます。`));
			setProp(_el$9, "style", para);
			insertNode(_el$1, createTextNode(`これは二つ目の段落です。view / text は CSS \`user-select\` と同型で既定選択可なので、\`selectable\` を宣言しなくても選択できます。`));
			setProp(_el$1, "style", para);
			insertNode(_el$11, _el$12);
			setProp(_el$11, "user-select", "none");
			insertNode(_el$12, createTextNode(`このキャプションは user-select: none の view に包まれているので、本文を全選択しても選択対象に入りません。`));
			effect((_p$) => {
				var _v$5 = {
					display: "flex",
					flexDirection: "column",
					gap: 8,
					padding: 12,
					backgroundColor: props.colors.panel2,
					borderRadius: 12,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: props.colors.line
				}, _v$6 = {
					color: props.colors.muted,
					fontSize: 11
				};
				_v$5 !== _p$.e && (_p$.e = setProp(_el$8, "style", _v$5, _p$.e));
				_v$6 !== _p$.t && (_p$.t = setProp(_el$12, "style", _v$6, _p$.t));
				return _p$;
			}, {
				e: void 0,
				t: void 0
			});
			return _el$8;
		})();
	}
	function EmptyState(props) {
		return (() => {
			var _el$14 = createElement("view"), _el$15 = createElement("text");
			insertNode(_el$14, _el$15);
			insertNode(_el$15, createTextNode(`表示するタスクがありません`));
			effect((_p$) => {
				var _v$7 = {
					height: 96,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					backgroundColor: props.colors.panel2,
					borderRadius: 12,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: props.colors.line
				}, _v$8 = {
					color: props.colors.muted,
					fontSize: 14
				};
				_v$7 !== _p$.e && (_p$.e = setProp(_el$14, "style", _v$7, _p$.e));
				_v$8 !== _p$.t && (_p$.t = setProp(_el$15, "style", _v$8, _p$.t));
				return _p$;
			}, {
				e: void 0,
				t: void 0
			});
			return _el$14;
		})();
	}
	function Footer(props) {
		return (() => {
			var _el$17 = createElement("view"), _el$18 = createElement("text"), _el$19 = createElement("view"), _el$20 = createElement("text"), _el$22 = createElement("button");
			insertNode(_el$17, _el$18);
			insertNode(_el$17, _el$19);
			setProp(_el$17, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				justifyContent: "space-between"
			});
			insert(_el$18, () => `${props.percent}% 完了`);
			insertNode(_el$19, _el$20);
			insertNode(_el$19, _el$22);
			setProp(_el$19, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				gap: 12
			});
			insertNode(_el$20, createTextNode(`クリックで完了 / × で削除`));
			insertNode(_el$22, createTextNode(`完了を消す`));
			effect((_p$) => {
				var _v$9 = {
					color: props.colors.muted,
					fontSize: 13
				}, _v$0 = {
					color: props.colors.quiet,
					fontSize: 11
				}, _v$1 = {
					height: 30,
					paddingLeft: 12,
					paddingRight: 12,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					backgroundColor: props.colors.panel2,
					defaultColor: props.colors.text,
					borderRadius: 8,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: props.colors.line,
					defaultFontSize: 12,
					...EASE,
					":hover": {
						backgroundColor: props.colors.panel3,
						borderColor: props.colors.danger,
						defaultColor: props.colors.danger
					}
				}, _v$10 = props.onClearDone;
				_v$9 !== _p$.e && (_p$.e = setProp(_el$18, "style", _v$9, _p$.e));
				_v$0 !== _p$.t && (_p$.t = setProp(_el$20, "style", _v$0, _p$.t));
				_v$1 !== _p$.a && (_p$.a = setProp(_el$22, "style", _v$1, _p$.a));
				_v$10 !== _p$.o && (_p$.o = setProp(_el$22, "onClick", _v$10, _p$.o));
				return _p$;
			}, {
				e: void 0,
				t: void 0,
				a: void 0,
				o: void 0
			});
			return _el$17;
		})();
	}
	//#endregion
	//#region src/components/TodoRow.tsx
	function iconButton(p) {
		return {
			width: 30,
			height: 30,
			display: "flex",
			alignItems: "center",
			justifyContent: "center",
			backgroundColor: p.panel,
			defaultColor: p.muted,
			borderRadius: 8,
			borderWidth: 1,
			borderStyle: "solid",
			borderColor: p.line,
			defaultFontSize: 14,
			...EASE,
			":hover": {
				backgroundColor: p.panel3,
				borderColor: p.line,
				defaultColor: p.text
			}
		};
	}
	function TodoRow(props) {
		const done = props.todo.done;
		const p = props.colors;
		return (() => {
			var _el$ = createElement("view"), _el$2 = createElement("button"), _el$3 = createElement("view"), _el$4 = createElement("view"), _el$5 = createElement("text"), _el$6 = createElement("button");
			insertNode(_el$, _el$2);
			insertNode(_el$, _el$3);
			insertNode(_el$, _el$4);
			insertNode(_el$, _el$5);
			insertNode(_el$, _el$6);
			insert(_el$2, done ? "✓" : " ");
			setProp(_el$4, "style", {
				flexGrow: 1,
				display: "flex",
				flexDirection: "column"
			});
			insert(_el$4, (() => {
				var _c$ = memo(() => !!props.editing);
				return () => _c$() ? (() => {
					var _el$8 = createElement("text-input");
					setProp(_el$8, "onInput", (event) => props.onEditInput(event.value ?? ""));
					setProp(_el$8, "onKeyDown", (event) => {
						const action = editKeyAction(event.key ?? "");
						if (action === "commit") props.onCommitEdit();
						else if (action === "cancel") props.onCancelEdit();
					});
					effect((_p$) => {
						var _v$8 = props.editDraft, _v$9 = {
							...inputStyle(p),
							height: 30,
							fontSize: 15
						}, _v$0 = props.onCommitEdit;
						_v$8 !== _p$.e && (_p$.e = setProp(_el$8, "value", _v$8, _p$.e));
						_v$9 !== _p$.t && (_p$.t = setProp(_el$8, "style", _v$9, _p$.t));
						_v$0 !== _p$.a && (_p$.a = setProp(_el$8, "onBlur", _v$0, _p$.a));
						return _p$;
					}, {
						e: void 0,
						t: void 0,
						a: void 0
					});
					return _el$8;
				})() : (() => {
					var _el$9 = createElement("button");
					insert(_el$9, () => props.todo.text);
					effect((_p$) => {
						var _v$1 = {
							display: "flex",
							alignItems: "center",
							backgroundColor: "transparent",
							defaultColor: done ? p.quiet : p.ink,
							defaultFontSize: 15,
							borderWidth: 0,
							borderStyle: "solid",
							...EASE,
							":hover": { defaultColor: p.accent }
						}, _v$10 = props.onBeginEdit;
						_v$1 !== _p$.e && (_p$.e = setProp(_el$9, "style", _v$1, _p$.e));
						_v$10 !== _p$.t && (_p$.t = setProp(_el$9, "onClick", _v$10, _p$.t));
						return _p$;
					}, {
						e: void 0,
						t: void 0
					});
					return _el$9;
				})();
			})());
			setProp(_el$5, "styleVariants", [{
				condition: { maxWidth: 719 },
				style: { display: "none" }
			}]);
			insert(_el$5, () => `優先度 ${PRIORITY_LABEL[props.todo.prio]}`);
			insert(_el$, (() => {
				var _c$2 = memo(() => !!props.reorderable);
				return () => _c$2() ? (() => {
					var _el$0 = createElement("view"), _el$1 = createElement("button"), _el$11 = createElement("button");
					insertNode(_el$0, _el$1);
					insertNode(_el$0, _el$11);
					setProp(_el$0, "style", {
						display: "flex",
						flexDirection: "row",
						alignItems: "center",
						gap: 4
					});
					insertNode(_el$1, createTextNode(`↑`));
					insertNode(_el$11, createTextNode(`↓`));
					effect((_p$) => {
						var _v$11 = iconButton(p), _v$12 = props.onMoveUp, _v$13 = iconButton(p), _v$14 = props.onMoveDown;
						_v$11 !== _p$.e && (_p$.e = setProp(_el$1, "style", _v$11, _p$.e));
						_v$12 !== _p$.t && (_p$.t = setProp(_el$1, "onClick", _v$12, _p$.t));
						_v$13 !== _p$.a && (_p$.a = setProp(_el$11, "style", _v$13, _p$.a));
						_v$14 !== _p$.o && (_p$.o = setProp(_el$11, "onClick", _v$14, _p$.o));
						return _p$;
					}, {
						e: void 0,
						t: void 0,
						a: void 0,
						o: void 0
					});
					return _el$0;
				})() : null;
			})(), _el$6);
			insertNode(_el$6, createTextNode(`×`));
			effect((_p$) => {
				var _v$ = {
					display: "flex",
					flexDirection: "row",
					alignItems: "center",
					gap: 12,
					padding: 12,
					backgroundColor: p.panel2,
					borderRadius: 12,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: p.line,
					opacity: done ? .62 : 1,
					boxShadow: [{
						offsetX: 0,
						offsetY: 2,
						blur: 6,
						spread: -1,
						color: p.shadow,
						inset: false
					}],
					...EASE,
					":hover": {
						backgroundColor: p.panel3,
						borderColor: p.line,
						boxShadow: [{
							offsetX: 0,
							offsetY: 10,
							blur: 24,
							spread: -4,
							color: p.shadow,
							inset: false
						}]
					}
				}, _v$2 = {
					width: 24,
					height: 24,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					backgroundColor: done ? p.success : p.panel,
					defaultColor: p.black,
					borderRadius: 7,
					borderWidth: 1,
					borderStyle: "solid",
					borderColor: done ? p.success : p.line,
					defaultFontSize: 14,
					boxShadow: done ? glow(`${p.success}66`) : [],
					...EASE,
					":hover": { borderColor: p.success }
				}, _v$3 = props.onToggle, _v$4 = {
					width: 10,
					height: 10,
					backgroundColor: priorityTone(p, props.todo.prio),
					borderRadius: 999
				}, _v$5 = {
					color: p.quiet,
					fontSize: 11
				}, _v$6 = {
					...iconButton(p),
					":hover": {
						backgroundColor: p.dangerBg,
						borderColor: p.danger,
						defaultColor: p.danger
					}
				}, _v$7 = props.onRemove;
				_v$ !== _p$.e && (_p$.e = setProp(_el$, "style", _v$, _p$.e));
				_v$2 !== _p$.t && (_p$.t = setProp(_el$2, "style", _v$2, _p$.t));
				_v$3 !== _p$.a && (_p$.a = setProp(_el$2, "onClick", _v$3, _p$.a));
				_v$4 !== _p$.o && (_p$.o = setProp(_el$3, "style", _v$4, _p$.o));
				_v$5 !== _p$.i && (_p$.i = setProp(_el$5, "style", _v$5, _p$.i));
				_v$6 !== _p$.n && (_p$.n = setProp(_el$6, "style", _v$6, _p$.n));
				_v$7 !== _p$.s && (_p$.s = setProp(_el$6, "onClick", _v$7, _p$.s));
				return _p$;
			}, {
				e: void 0,
				t: void 0,
				a: void 0,
				o: void 0,
				i: void 0,
				n: void 0,
				s: void 0
			});
			return _el$;
		})();
	}
	//#endregion
	//#region src/components/Toolbar.tsx
	function chipStyle(p, active) {
		return {
			height: 30,
			paddingLeft: 12,
			paddingRight: 12,
			display: "flex",
			alignItems: "center",
			justifyContent: "center",
			backgroundColor: active ? p.accent : p.panel2,
			defaultColor: active ? p.black : p.text,
			borderRadius: 999,
			borderWidth: 1,
			borderStyle: "solid",
			borderColor: active ? p.accent : p.line,
			defaultFontSize: 12,
			boxShadow: active ? glow(`${p.accent}44`) : [],
			...EASE,
			":hover": {
				backgroundColor: active ? p.accent : p.panel3,
				borderColor: active ? p.accent : p.line
			}
		};
	}
	function Toolbar(props) {
		return (() => {
			var _el$ = createElement("view"), _el$2 = createElement("text"), _el$4 = createElement("view"), _el$5 = createElement("text");
			insertNode(_el$, _el$2);
			insertNode(_el$, _el$4);
			insertNode(_el$, _el$5);
			setProp(_el$, "style", {
				display: "flex",
				flexDirection: "row",
				alignItems: "center",
				flexWrap: "wrap",
				gap: 8,
				paddingTop: 10,
				paddingBottom: 10
			});
			insertNode(_el$2, createTextNode(`表示`));
			insert(_el$, () => FILTERS.map((item) => (() => {
				var _el$7 = createElement("button");
				setProp(_el$7, "onClick", () => props.onFilter(item.value));
				insert(_el$7, () => item.label);
				effect((_$p) => setProp(_el$7, "style", chipStyle(props.colors, props.filter === item.value), _$p));
				return _el$7;
			})()), _el$4);
			insertNode(_el$5, createTextNode(`並び`));
			insert(_el$, () => SORTS.map((item) => (() => {
				var _el$8 = createElement("button");
				setProp(_el$8, "onClick", () => props.onSort(item.value));
				insert(_el$8, () => item.label);
				effect((_$p) => setProp(_el$8, "style", chipStyle(props.colors, props.sort === item.value), _$p));
				return _el$8;
			})()), null);
			effect((_p$) => {
				var _v$ = {
					color: props.colors.quiet,
					fontSize: 12
				}, _v$2 = {
					width: 1,
					height: 18,
					marginLeft: 4,
					marginRight: 4,
					backgroundColor: props.colors.line
				}, _v$3 = {
					color: props.colors.quiet,
					fontSize: 12
				};
				_v$ !== _p$.e && (_p$.e = setProp(_el$2, "style", _v$, _p$.e));
				_v$2 !== _p$.t && (_p$.t = setProp(_el$4, "style", _v$2, _p$.t));
				_v$3 !== _p$.a && (_p$.a = setProp(_el$5, "style", _v$3, _p$.a));
				return _p$;
			}, {
				e: void 0,
				t: void 0,
				a: void 0
			});
			return _el$;
		})();
	}
	//#endregion
	//#region src/App.tsx
	function seedTodos() {
		return SEED.map((todo) => ({ ...todo }));
	}
	function TodoApp(props) {
		const [page, setPage] = createSignal(new URLSearchParams(window.location.search).get("page") === "gallery" ? "gallery" : "tasks");
		const [todos, setTodos] = createSignal(seedTodos());
		const [filter, setFilter] = createSignal("all");
		const [sort, setSort] = createSignal("manual");
		const [draftPrio, setDraftPrio] = createSignal(2);
		const [draft, setDraft] = createSignal("");
		const [editingId, setEditingId] = createSignal(null);
		const [editDraft, setEditDraft] = createSignal("");
		let nextId = 1e3;
		const initialPrefs = loadTheme(window.localStorage);
		const [theme, setTheme] = createSignal(initialPrefs.theme);
		const [accent, setAccent] = createSignal(initialPrefs.accent);
		const colors = createMemo(() => palette(theme(), accent()));
		createEffect(() => saveTheme(window.localStorage, {
			theme: theme(),
			accent: accent()
		}));
		createEffect(() => {
			const p = colors();
			const root = document.documentElement.style;
			root.setProperty("--rsw-bg", p.rail);
			root.setProperty("--rsw-line", p.line);
			root.setProperty("--rsw-text", p.muted);
			root.setProperty("--rsw-ink", p.ink);
			root.setProperty("--rsw-hover", p.panel3);
			root.setProperty("--rsw-on-accent", p.black);
			root.setProperty("--rsw-accent", p.accent);
		});
		const toggleTheme = () => setTheme((current) => current === "dark" ? "light" : "dark");
		const visible = createMemo(() => visibleTodos(todos(), filter(), sort()));
		const summary = createMemo(() => completion(todos()));
		const addTask = () => {
			const text = draft();
			if (!text.trim()) return;
			setTodos(add(todos(), {
				id: nextId++,
				text,
				prio: draftPrio()
			}));
			setDraft("");
		};
		const toggle = (id) => setTodos(toggleDone(todos(), id));
		const removeTask = (id) => setTodos(remove(todos(), id));
		const clearCompleted = () => setTodos(clearDone(todos()));
		const moveTaskUp = (id) => setTodos(moveUp(todos(), id));
		const moveTaskDown = (id) => setTodos(moveDown(todos(), id));
		const beginEdit = (todo) => {
			setEditingId(todo.id);
			setEditDraft(todo.text);
		};
		const commitEdit = () => {
			const id = editingId();
			if (id === null) return;
			setTodos(editText(todos(), id, editDraft()));
			setEditingId(null);
		};
		const cancelEdit = () => setEditingId(null);
		return (() => {
			var _el$ = createElement("view");
			insert(_el$, createComponent(AppBar, {
				get page() {
					return page();
				},
				setPage,
				get detected() {
					return props.detected;
				},
				get colors() {
					return colors();
				},
				get theme() {
					return theme();
				},
				get accent() {
					return accent();
				},
				onToggleTheme: toggleTheme,
				onAccent: setAccent
			}), null);
			insert(_el$, (() => {
				var _c$ = memo(() => page() === "gallery");
				return () => _c$() ? createComponent(CssGallery, { get colors() {
					return colors();
				} }) : (() => {
					var _el$2 = createElement("scroll-view"), _el$3 = createElement("view"), _el$4 = createElement("view"), _el$5 = createElement("view");
					insertNode(_el$2, _el$3);
					setProp(_el$2, "styleVariants", [{
						condition: { maxWidth: 719 },
						style: {
							paddingTop: 16,
							paddingBottom: 16,
							paddingLeft: 12,
							paddingRight: 12
						}
					}]);
					insertNode(_el$3, _el$4);
					insertNode(_el$3, _el$5);
					setProp(_el$3, "styleVariants", [{
						condition: { maxWidth: 719 },
						style: {
							padding: 14,
							gap: 12,
							borderRadius: 12
						}
					}]);
					insert(_el$3, createComponent(Header, {
						get colors() {
							return colors();
						},
						get remaining() {
							return summary().remaining;
						},
						get total() {
							return summary().total;
						},
						get percent() {
							return summary().percent;
						}
					}), _el$4);
					insert(_el$3, createComponent(SelectableNote, { get colors() {
						return colors();
					} }), _el$4);
					insert(_el$3, createComponent(AddForm, {
						get colors() {
							return colors();
						},
						get draft() {
							return draft();
						},
						get prio() {
							return draftPrio();
						},
						onInput: setDraft,
						onPrio: setDraftPrio,
						onAdd: addTask
					}), _el$4);
					insert(_el$3, createComponent(Toolbar, {
						get colors() {
							return colors();
						},
						get filter() {
							return filter();
						},
						get sort() {
							return sort();
						},
						onFilter: setFilter,
						onSort: setSort
					}), _el$4);
					setProp(_el$4, "style", {
						display: "flex",
						flexDirection: "column",
						gap: 8
					});
					insert(_el$4, (() => {
						var _c$2 = memo(() => visible().length === 0);
						return () => _c$2() ? createComponent(EmptyState, { get colors() {
							return colors();
						} }) : visible().map((todo) => createComponent(TodoRow, {
							get colors() {
								return colors();
							},
							todo,
							get reorderable() {
								return canReorder(sort());
							},
							get editing() {
								return editingId() === todo.id;
							},
							get editDraft() {
								return editDraft();
							},
							onToggle: () => toggle(todo.id),
							onRemove: () => removeTask(todo.id),
							onBeginEdit: () => beginEdit(todo),
							onEditInput: setEditDraft,
							onCommitEdit: commitEdit,
							onCancelEdit: cancelEdit,
							onMoveUp: () => moveTaskUp(todo.id),
							onMoveDown: () => moveTaskDown(todo.id)
						}));
					})());
					insert(_el$3, createComponent(Footer, {
						get colors() {
							return colors();
						},
						get percent() {
							return summary().percent;
						},
						onClearDone: clearCompleted
					}), null);
					effect((_p$) => {
						var _v$ = {
							flexGrow: 1,
							width: "100%",
							height: "100%",
							display: "flex",
							flexDirection: "column",
							alignItems: "center",
							paddingTop: 28,
							paddingBottom: 28,
							paddingLeft: 16,
							paddingRight: 16,
							backgroundColor: colors().bg
						}, _v$2 = {
							width: 620,
							maxWidth: "100%",
							display: "flex",
							flexDirection: "column",
							gap: 16,
							padding: 22,
							backgroundColor: colors().panel,
							borderRadius: 18,
							borderWidth: 1,
							borderStyle: "solid",
							borderColor: colors().line,
							boxShadow: [{
								offsetX: 0,
								offsetY: 18,
								blur: 40,
								spread: -8,
								color: colors().shadow,
								inset: false
							}]
						}, _v$3 = {
							height: 1,
							backgroundColor: colors().line
						};
						_v$ !== _p$.e && (_p$.e = setProp(_el$2, "style", _v$, _p$.e));
						_v$2 !== _p$.t && (_p$.t = setProp(_el$3, "style", _v$2, _p$.t));
						_v$3 !== _p$.a && (_p$.a = setProp(_el$5, "style", _v$3, _p$.a));
						return _p$;
					}, {
						e: void 0,
						t: void 0,
						a: void 0
					});
					return _el$2;
				})();
			})(), null);
			effect((_$p) => setProp(_el$, "style", {
				width: "100%",
				height: "100%",
				display: "flex",
				flexDirection: "column",
				backgroundColor: colors().bg,
				defaultColor: colors().text,
				defaultFontSize: 14,
				defaultFontFamily: "Inter, Segoe UI, system-ui, sans-serif"
			}, _$p));
			return _el$;
		})();
	}
	//#endregion
	//#region src/main.android.tsx
	var raw = globalThis.__hayateHost;
	if (raw === void 0) throw new Error("Android: globalThis.__hayateHost (native RawHayate) が注入されていません");
	var detected = {
		mode: "Canvas",
		backend: "vello",
		source: "query",
		renderer: "vello"
	};
	var handle = createAndroidCanvasRenderer(raw);
	renderTsubame(() => createComponent(TodoApp, { detected }), handle.renderer);
	globalThis.__tsubame = {
		pumpFrame: (timestampMs) => handle.pumpFrame(timestampMs),
		resize: (width, height, scale) => handle.resize(width, height, scale),
		stop: () => handle.stop()
	};
	//#endregion
})();
