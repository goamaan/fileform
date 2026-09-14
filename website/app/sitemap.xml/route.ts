const origin='https://fileform.amaangokak18.chatgpt.site';
const paths=['/','/formats','/download','/releases','/docs','/licences','/privacy','/terms','/contact'];
export function GET(){return new Response('<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">'+paths.map(path=>'<url><loc>'+origin+path+'</loc></url>').join('')+'</urlset>',{headers:{'Content-Type':'application/xml; charset=utf-8'}})}
