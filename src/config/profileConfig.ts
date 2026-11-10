import {siteSnapshot} from "../utils/site-snapshot";
import type { ProfileConfig } from "../types/profileConfig";

export const profileConfig: ProfileConfig = {
	avatar: "/assets/ai-xiaoai-avatar.png",
	name: "你的昵称",
	bio: "请填写自己的简介",
	links: [],
};

if(siteSnapshot.profile){Object.assign(profileConfig,siteSnapshot.profile,{links:siteSnapshot.profile.links.map(l=>({...l,icon:l.icon||"material-symbols:link"}))});}
