import {siteSnapshot} from "../utils/site-snapshot";
import type { FooterConfig } from "../types/footerConfig";

export const footerConfig: FooterConfig = {
	// 是否启用Footer HTML注入功能
	enable: true,
	records: [], // Add your own registration links if applicable.
};

// 直接编辑 config/FooterConfig.html 文件来添加备案号等自定义内容

if(siteSnapshot.footer){footerConfig.records=siteSnapshot.footer.records.map(l=>({label:l.name,url:l.url}));}
