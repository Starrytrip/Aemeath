import type { SakuraConfig } from "@/types/effectsConfig";

const random = (min: number, max: number): number =>
	min + Math.random() * (max - min);

/** Persistent transform/opacity animations keep moving during route DOM work. */
export class SakuraManager {
	config: SakuraConfig;
	layer: HTMLDivElement | null = null;
	animations: Animation[] = [];
	isRunning = false;
	isPaused = false;
	private width = 0;
	private height = 0;
	private imageUrl: string;

	constructor(config: SakuraConfig, imageUrl: string) {
		this.config = config;
		this.imageUrl = imageUrl;
	}

	init(): void {
		if (!this.config.enable || this.isRunning) return;
		this.layer = document.createElement("div");
		this.layer.id = "sakura-effect";
		this.layer.setAttribute("aria-hidden", "true");
		this.layer.style.cssText = `position:fixed;inset:0;overflow:hidden;pointer-events:none;contain:strict;z-index:${this.config.zIndex}`;
		document.body.append(this.layer);
		this.isRunning = true;
		this.createPetals();
		window.addEventListener("resize", this.handleResize, { passive: true });
	}

	private createPetals(): void {
		if (!this.layer) return;
		for (const animation of this.animations) animation.cancel();
		this.animations = [];
		this.layer.replaceChildren();
		this.width = window.innerWidth;
		this.height = window.innerHeight;
		const count = Math.max(0, Math.min(100, this.config.sakuraNum));
		for (let index = 0; index < count; index++) {
			const size = 40 * random(this.config.size.min, this.config.size.max);
			const distance = this.height + size * 2;
			// Existing speed settings use pixels per frame at roughly 60 fps.
			const duration =
				(distance /
					Math.max(
						1,
						random(
							this.config.speed.vertical.min,
							this.config.speed.vertical.max,
						) * 60,
					)) *
				1000;
			const drift =
				(random(
					this.config.speed.horizontal.min,
					this.config.speed.horizontal.max,
				) *
					60 *
					duration) /
				1000;
			const x = random(-Math.max(0, drift), this.width - Math.min(0, drift));
			const angle = random(0, 360);
			const rotation =
				(((this.config.speed.rotation * 60 * duration) / 1000) * 180) / Math.PI;
			const opacity = random(this.config.opacity.min, this.config.opacity.max);
			const endOpacity = Math.max(
				0,
				opacity - ((this.config.speed.fadeSpeed * 0.01 * duration) / 1000) * 60,
			);
			const petal = document.createElement("span");
			petal.style.cssText = `position:absolute;left:0;top:0;width:${size}px;height:${size}px;will-change:transform,opacity;background-size:contain;background-repeat:no-repeat`;
			petal.style.backgroundImage = `url("${this.imageUrl}")`;
			this.layer.append(petal);
			const animation = petal.animate(
				[
					{
						transform: `translate3d(${x}px,${-size}px,0) rotate(${angle}deg)`,
						opacity,
					},
					{
						transform: `translate3d(${x + drift}px,${this.height + size}px,0) rotate(${angle + rotation}deg)`,
						opacity: endOpacity,
					},
				],
				{
					duration,
					iterations:
						this.config.limitTimes < 0
							? Number.POSITIVE_INFINITY
							: this.config.limitTimes + 1,
					easing: "linear",
					fill: "both",
				},
			);
			// Distribute petals throughout the viewport from the first paint.
			animation.currentTime = random(0, duration);
			if (this.isPaused) animation.pause();
			this.animations.push(animation);
		}
	}

	private handleResize = (): void => {
		if (
			this.width !== window.innerWidth ||
			this.height !== window.innerHeight
		) {
			this.createPetals();
		}
	};

	pause(): void {
		if (!this.isRunning || this.isPaused) return;
		this.isPaused = true;
		for (const animation of this.animations) animation.pause();
	}

	resume(): void {
		if (!this.isRunning || !this.isPaused) return;
		this.isPaused = false;
		for (const animation of this.animations) animation.play();
	}

	stop(): void {
		for (const animation of this.animations) animation.cancel();
		this.animations = [];
		this.layer?.remove();
		this.layer = null;
		window.removeEventListener("resize", this.handleResize);
		this.isRunning = false;
		this.isPaused = false;
	}

	updateConfig(config: SakuraConfig): void {
		const restart = this.isRunning;
		this.stop();
		this.config = config;
		if (restart && config.enable) this.init();
	}

	getIsRunning(): boolean {
		return this.isRunning;
	}
}
