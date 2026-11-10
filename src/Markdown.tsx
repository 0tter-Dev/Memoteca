import ReactMarkdown, { type Components } from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import remarkGfm from "remark-gfm";

/**
 * Links e imagens nas respostas do modelo **não são clicáveis nem carregam nada**: o endereço
 * aparece escrito, para o usuário copiar se quiser. Assim uma resposta nunca leva a uma página
 * (nem a uma requisição de imagem) sem o usuário querer. HTML cru na resposta também não é
 * interpretado: `react-markdown` não renderiza HTML sem um plugin que o libere.
 */
const componentes: Components = {
  a({ href, children }) {
    const soOEndereco = typeof children === "string" && children === href;
    return (
      <span className="link-como-texto">
        {children}
        {href && !soOEndereco && <> ({href})</>}
      </span>
    );
  },
  img({ alt }) {
    return (
      <span className="link-como-texto">[imagem{alt ? `: ${alt}` : ""}]</span>
    );
  },
};

interface Props {
  texto: string;
}

/** Markdown das respostas, com tabelas e listas de tarefas (GFM) e código realçado. */
function Markdown({ texto }: Props) {
  return (
    <div className="markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeHighlight, { detect: false }]]}
        components={componentes}
      >
        {texto}
      </ReactMarkdown>
    </div>
  );
}

export default Markdown;
