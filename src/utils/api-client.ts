// 后端 API 客户端：统一处理访问令牌、刷新 Cookie、访客标识与错误信息。

import { migrateBrandStorage } from "./brand-storage";

if (typeof window !== "undefined") {
	try {
		migrateBrandStorage(window.localStorage);
	} catch {
		/* Storage may be blocked. */
	}
}

const API_BASE = "/api/v1";
const TOKEN_KEY = "yukina.access-token";
const VISITOR_KEY = "yukina.visitor-id";

export interface AuthUser {
	id: number;
	username: string;
	email: string | null;
	is_admin: boolean;
	nickname: string | null;
	gender: string | null;
	birthday: string | null;
	job: string | null;
	hobbies: string | null;
	motto: string | null;
	bio: string | null;
	avatar_url: string | null;
	cover_url: string | null;
}

/** 个人资料可编辑字段：缺省表示不修改，空字符串表示清空。 */
export interface ProfileUpdateInput {
	nickname?: string;
	gender?: string;
	birthday?: string;
	job?: string;
	hobbies?: string;
	motto?: string;
	bio?: string;
}

export interface CommentRow {
	id: number;
	page: string;
	nick: string;
	website: string | null;
	content: string;
	parent_id: number | null;
	avatar_url: string | null;
	is_admin: boolean;
	/** 来源地：境内省市 / 境外国家 / 内网IP / 脱敏 IP */
	region: string | null;
	user_agent: string | null;
	likes: number;
	liked: boolean;
	created_at: string;
}

/** 评论点赞结果。 */
export interface CommentLikeResult {
	likes: number;
	liked: boolean;
}

export interface PostStats {
	page: string;
	views: number;
	likes: number;
	liked: boolean;
}

/** 后端返回的错误（响应体为 `{ detail }`）。 */
export class ApiError extends Error {
	status: number;

	constructor(status: number, message: string) {
		super(message);
		this.name = "ApiError";
		this.status = status;
	}
}

export function getAccessToken(): string | null {
	try {
		return localStorage.getItem(TOKEN_KEY);
	} catch {
		return null;
	}
}

export function setAccessToken(token: string | null): void {
	try {
		if (token) localStorage.setItem(TOKEN_KEY, token);
		else localStorage.removeItem(TOKEN_KEY);
	} catch {
		// 隐私模式下 localStorage 可能不可用，忽略即可
	}
}

/** 访客标识：持久化在 localStorage，用于阅读量/点赞去重。 */
export function getVisitorId(): string {
	try {
		const saved = localStorage.getItem(VISITOR_KEY);
		if (saved) return saved;
		const id =
			typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
				? crypto.randomUUID()
				: `visitor-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
		localStorage.setItem(VISITOR_KEY, id);
		return id;
	} catch {
		return "anonymous-visitor";
	}
}

let refreshPromise: Promise<boolean> | null = null;

/** 用 HttpOnly 刷新 Cookie 换新的访问令牌。 */
async function refreshSession(): Promise<boolean> {
	if (refreshPromise) return refreshPromise;
	refreshPromise = (async () => {
		try {
			const response = await fetch(`${API_BASE}/auth/refresh`, {
				method: "POST",
				credentials: "same-origin",
			});
			if (!response.ok) {
				setAccessToken(null);
				return false;
			}
			const data = (await response.json()) as { access_token?: string };
			if (!data.access_token) return false;
			setAccessToken(data.access_token);
			return true;
		} catch {
			return false;
		}
	})().finally(() => {
		refreshPromise = null;
	});
	return refreshPromise;
}

async function runRequest(
	path: string,
	init: RequestInit,
	token: string | null,
) {
	const headers = new Headers(init.headers);
	// 仅对字符串请求体补 JSON 头；FormData 交给浏览器自动生成 multipart 边界
	if (typeof init.body === "string" && !headers.has("Content-Type")) {
		headers.set("Content-Type", "application/json");
	}
	headers.set("X-Visitor-Id", getVisitorId());
	if (token) headers.set("Authorization", `Bearer ${token}`);
	return fetch(`${API_BASE}${path}`, {
		...init,
		headers,
		credentials: "same-origin",
	});
}

/** 发起 API 请求；访问令牌过期时自动尝试刷新一次。 */
export async function apiFetch<T>(
	path: string,
	init: RequestInit = {},
): Promise<T> {
	let response = await runRequest(path, init, getAccessToken());
	if (response.status === 401 && getAccessToken() && (await refreshSession())) {
		response = await runRequest(path, init, getAccessToken());
	}

	if (!response.ok) {
		let detail = `请求失败（${response.status}）`;
		try {
			const data = (await response.json()) as { detail?: string };
			if (data?.detail) detail = data.detail;
		} catch {
			// 响应体不是 JSON 时使用默认提示
		}
		throw new ApiError(response.status, detail);
	}

	if (response.status === 204) return undefined as T;
	return (await response.json()) as T;
}

/** 用户名或邮箱 + 密码登录，成功后保存访问令牌。 */
export async function login(account: string, password: string): Promise<void> {
	const data = await apiFetch<{ access_token: string }>("/auth/login", {
		method: "POST",
		body: JSON.stringify({ account, password }),
	});
	setAccessToken(data.access_token);
}

/** 获取当前登录用户。 */
export function fetchMe(): Promise<AuthUser> {
	return apiFetch<AuthUser>("/auth/me");
}

/** 更新当前用户资料，返回最新资料。 */
export function updateProfile(input: ProfileUpdateInput): Promise<AuthUser> {
	return apiFetch<AuthUser>("/profile", {
		method: "PATCH",
		body: JSON.stringify(input),
	});
}

/** 上传头像或封面（原始文件或前端裁剪后的 Blob），返回可直接使用的图片地址。 */
export async function uploadProfileImage(
	kind: "avatar" | "cover",
	image: Blob,
): Promise<string> {
	const form = new FormData();
	form.append("file", image, kind === "avatar" ? "avatar.jpg" : "cover.jpg");
	const data = await apiFetch<{ url: string }>(`/profile/${kind}`, {
		method: "POST",
		body: form,
	});
	return data.url;
}

/** 发送邮箱验证码：purpose 为 `register`（注册）或 `reset`（找回密码）。 */
export function sendEmailCode(
	email: string,
	purpose: "register" | "reset",
): Promise<{ detail: string; code_ttl_minutes?: number }> {
	return apiFetch("/auth/email/code", {
		method: "POST",
		body: JSON.stringify({ email, purpose }),
	});
}

/** 邮箱验证码校验通过后注册账号并自动登录。 */
export async function register(input: {
	username: string;
	email: string;
	code: string;
	password: string;
}): Promise<void> {
	const data = await apiFetch<{ access_token: string }>("/auth/register", {
		method: "POST",
		body: JSON.stringify(input),
	});
	setAccessToken(data.access_token);
}

/** 使用邮箱验证码设置新密码。 */
export function resetPassword(
	email: string,
	code: string,
	password: string,
): Promise<{ detail: string }> {
	return apiFetch("/auth/password/reset", {
		method: "POST",
		body: JSON.stringify({ email, code, password }),
	});
}

/** 切换评论点赞状态（按访客标识去重）。 */
export function toggleCommentLike(id: number): Promise<CommentLikeResult> {
	return apiFetch<CommentLikeResult>(`/comments/${id}/like`, {
		method: "POST",
	});
}

/** 上传评论配图（GIF 保留动图），返回可直接插入的图片地址。 */
export async function uploadCommentImage(file: File): Promise<string> {
	const form = new FormData();
	form.append("file", file, file.name || "comment-image");
	const data = await apiFetch<{ url: string }>("/comments/upload", {
		method: "POST",
		body: form,
	});
	return data.url;
}

/** 退出登录并清除本地访问令牌。 */
export async function logout(): Promise<void> {
	try {
		await apiFetch("/auth/logout", { method: "POST" });
	} finally {
		setAccessToken(null);
	}
}
