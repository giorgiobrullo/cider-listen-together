export const prerender = true;

const site = 'https://www.cidertogether.app';

export function GET() {
	const lastmod = new Date().toISOString().split('T')[0];
	const sitemap = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
	<url>
		<loc>${site}</loc>
		<lastmod>${lastmod}</lastmod>
		<changefreq>weekly</changefreq>
		<priority>1.0</priority>
	</url>
</urlset>`;

	return new Response(sitemap, {
		headers: {
			'Content-Type': 'application/xml'
		}
	});
}
