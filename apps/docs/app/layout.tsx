import { Footer, Layout, Navbar } from "nextra-theme-docs";
import { Head } from "nextra/components";
import { getPageMap } from "nextra/page-map";
import "nextra-theme-docs/style.css";

export const metadata = {
  title: {
    default: "OSSM-rs Docs",
    template: "%s — OSSM-rs Docs",
  },
  description: "Documentation for OSSM-rs.",
};

const navbar = (
  <Navbar logo={<b>ossm-rs</b>} projectLink="https://github.com/ossm-rs/ossm" />
);

const footer = <Footer>OSSM-rs · {new Date().getFullYear()}</Footer>;

export default async function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" dir="ltr" suppressHydrationWarning>
      <Head />
      <body>
        <Layout
          navbar={navbar}
          pageMap={await getPageMap()}
          docsRepositoryBase="https://github.com/ossm-rs/ossm/tree/main/docs"
          footer={footer}
        >
          {children}
        </Layout>
      </body>
    </html>
  );
}
