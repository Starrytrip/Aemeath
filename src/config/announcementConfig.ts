import {siteSnapshot} from "../utils/site-snapshot";
import type { AnnouncementConfig } from "../types/announcementConfig";

export const announcementConfig: AnnouncementConfig = {
	title: "欢迎使用 Yukina",
	content: "请先填写 src/config 中的站点配置，再添加自己的文章和数据。",
	closable: true,
	links: [
		{
			enable: true,
			text: "配置说明",
			url: "https://github.com/Jarvis0227/Aemeath#内容与配置",
			external: true,
		},
	],
};

if(siteSnapshot.announcement){const a=siteSnapshot.announcement;Object.assign(announcementConfig,{title:a.title,content:a.content,closable:a.closable,links:a.links.map(l=>({text:l.name,url:l.url,enable:l.enabled!==false,external:l.external}))});}
