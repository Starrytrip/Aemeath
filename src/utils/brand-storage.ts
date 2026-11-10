/** Preserve browser preferences when upgrading from the previous project name. */
export function migrateBrandStorage(storage: Storage): void {
	const keys = Array.from({ length: storage.length }, (_, index) =>
		storage.key(index),
	);
	for (const key of keys) {
		if (!key || !/^aemeath[.:-]/.test(key)) continue;
		const next = key.replace(/^aemeath/, "yukina");
		if (storage.getItem(next) === null) {
			const value = storage.getItem(key);
			if (value !== null) storage.setItem(next, value);
		}
		storage.removeItem(key);
	}
}
